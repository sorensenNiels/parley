//! A TN5250 session, without I/O: [`Session::receive`] takes the bytes the host sent,
//! [`Session::key`] takes the operator's keys, and [`Session::take_outbound`] gives the bytes
//! to send back. [`crate::net::Connection`] runs one over a socket.

mod checks;
mod choices;
mod datastream;
mod gui;
mod input;
mod mouse;
mod playback;
mod reply;
mod typeahead;

pub use typeahead::TYPE_AHEAD_KEYS;

use serde::{Deserialize, Serialize};

pub use input::{Aid, Key};
pub use mouse::{MouseAction, MouseButton, MouseEvent};
pub use playback::{MACRO_TIMEOUT, MacroState};

use crate::macros::Step;
use crate::record::{self, Opcode};
use crate::screen::{Screen, ScreenSize, Style};
use crate::telnet::{Received, Telnet};
use crate::{CodePage, Error, Result};

/// TN5250's usual port over plain telnet (RFC 2877).
pub const TELNET_PORT: u16 = 23;
/// TN5250's usual port over TLS on IBM i (R-NET-2).
pub const TLS_PORT: u16 = 992;

/// Where and how to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    /// A PEM file with a CA certificate to trust besides the operating system's, for hosts whose
    /// certificate comes from a company CA (R-NET-2). Empty: the operating system's only.
    pub ca_file: String,
    pub code_page: CodePage,
    /// The largest screen the session offers; the host switches to it when a program needs it.
    pub screen_size: ScreenSize,
    /// The IBM i device name to ask for (R-NET-3). Empty: the host makes one up (`QPADEVxxxx`).
    pub device_name: String,
    /// Send a telnet No Operation after `keep_alive_seconds` without traffic (R-NET-5).
    pub keep_alive: bool,
    /// How long without traffic before a keep-alive goes, in seconds (R-NET-11); see
    /// [`SessionConfig::keep_alive_interval`].
    pub keep_alive_seconds: u16,
    /// Claim IBM i's GUI-like 5250 and draw its windows, selection fields and scroll bars
    /// (R-DS-10..12, D38, D57): the session dialog's "GUI elements (beta)", off by default.
    pub gui: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: TELNET_PORT,
            tls: false,
            ca_file: String::new(),
            code_page: CodePage::default(),
            screen_size: ScreenSize::default(),
            device_name: String::new(),
            keep_alive: false,
            keep_alive_seconds: KEEP_ALIVE_SECONDS,
            gui: false,
        }
    }
}

/// The keep-alive interval when none is chosen, and the range one may be (R-NET-11).
pub const KEEP_ALIVE_SECONDS: u16 = 60;
pub const KEEP_ALIVE_RANGE: std::ops::RangeInclusive<u16> = 10..=3600;

impl SessionConfig {
    /// The keep-alive interval, kept within [`KEEP_ALIVE_RANGE`] whatever the settings say.
    pub fn keep_alive_interval(&self) -> std::time::Duration {
        let seconds = self
            .keep_alive_seconds
            .clamp(*KEEP_ALIVE_RANGE.start(), *KEEP_ALIVE_RANGE.end());
        std::time::Duration::from_secs(u64::from(seconds))
    }

    /// The usual port for TN5250 with `tls` or without.
    pub const fn default_port(tls: bool) -> u16 {
        if tls { TLS_PORT } else { TELNET_PORT }
    }

    /// The terminal type sent during negotiation (RFC 2877 section 4).
    pub const fn terminal_type(&self) -> &'static str {
        match self.screen_size {
            ScreenSize::Standard => "IBM-3179-2",
            ScreenSize::Wide => "IBM-3477-FC",
        }
    }

    /// The user variables sent in NEW-ENVIRON (RFC 4777 section 5): the device name, and the
    /// keyboard type, code page and character set, so the host's job uses the session's CCSID.
    fn environment(&self) -> Vec<(String, String)> {
        let mut vars = Vec::new();
        let device = self.device_name.trim();
        if !device.is_empty() {
            vars.push(("DEVNAME".to_owned(), device.to_ascii_uppercase()));
        }
        let page = self.code_page;
        vars.push(("KBDTYPE".to_owned(), page.keyboard_type().to_owned()));
        vars.push(("CODEPAGE".to_owned(), page.ccsid().to_string()));
        vars.push(("CHARSET".to_owned(), page.character_set().to_string()));
        vars
    }
}

/// Why the keyboard is inhibited after the operator did something the field does not allow.
/// The operator presses Reset to go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorError {
    AlphaOnly,
    NumericOnly,
    DigitsOnly,
    NotKeyboardInput,
    NotInCodePage,
    NoRoomToInsert,
    FieldMinusNotAllowed,
    SignPosition,
    /// Enter with a mandatory enter field empty (R-KEY-10).
    MandatoryEnter,
    /// Field Exit past an empty mandatory enter field (R-KEY-10).
    MandatoryEnterExit,
    /// Leaving a mandatory fill field half filled (R-KEY-10).
    MandatoryFill,
    /// Field Exit in a half-filled mandatory fill field (R-KEY-10).
    MandatoryFillExit,
    /// A self-check field's number and check digit do not agree (R-KEY-10).
    CheckDigit,
    /// The host wrote an error message (Write Error Code).
    Host,
}

impl OperatorError {
    pub const fn message(self) -> &'static str {
        match self {
            Self::AlphaOnly => "Only letters, comma, period, minus and space are allowed here.",
            Self::NumericOnly => {
                "Only digits, plus, minus, comma, period and space are allowed here."
            }
            Self::DigitsOnly => "Only digits are allowed here.",
            Self::NotKeyboardInput => "This field cannot be typed in.",
            Self::NotInCodePage => "That character is not in the session's code page.",
            Self::NoRoomToInsert => "No room to insert: the field is full.",
            Self::FieldMinusNotAllowed => "Field- is only allowed in numeric fields.",
            Self::SignPosition => "This is the sign position: use Field+ or Field-.",
            Self::MandatoryEnter => "This field must have data before Enter.",
            Self::MandatoryEnterExit => "This field must have data before you leave it.",
            Self::MandatoryFill => "Fill the whole field before leaving it.",
            Self::MandatoryFillExit => {
                "Fill the whole field, or use Field Exit from its first position."
            }
            Self::CheckDigit => "The check digit does not match the number.",
            Self::Host => "See the message from the host.",
        }
    }

    /// The 5250 operator error code a standard 5250 display shows for it (R-KEY-9), as ASNA's
    /// "5250 Terminal Error Codes" list the standard codes; 0012 is also in IBM's SC30-3533-04
    /// (word wrap, "insert failed operator errors (0012)"). `None` for Parley's own check of
    /// the code page and for the host's own message.
    pub const fn code(self) -> Option<&'static str> {
        match self {
            Self::NotKeyboardInput => Some("0004"),
            Self::AlphaOnly => Some("0008"),
            Self::NumericOnly => Some("0009"),
            Self::DigitsOnly => Some("0010"),
            Self::SignPosition => Some("0011"),
            Self::NoRoomToInsert => Some("0012"),
            Self::FieldMinusNotAllowed => Some("0016"),
            Self::MandatoryEnter => Some("0007"),
            Self::MandatoryFill => Some("0014"),
            Self::CheckDigit => Some("0015"),
            Self::MandatoryFillExit => Some("0017"),
            Self::MandatoryEnterExit => Some("0021"),
            Self::NotInCodePage | Self::Host => None,
        }
    }

    /// The error as the status bar shows it: its code, if it has one, then its message.
    pub fn display(self) -> String {
        match self.code() {
            Some(code) => format!("{code} {}", self.message()),
            None => self.message().to_owned(),
        }
    }
}

/// The keyboard as the operator information area shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardState {
    Unlocked,
    /// Waiting for the host ("X SYSTEM").
    Locked,
    /// Inhibited by an error until the operator presses Reset.
    Error(OperatorError),
}

/// Which read command the host is waiting on: it decides what an AID key sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadKind {
    InputFields,
    MdtFields,
    MdtFieldsAlternate,
    /// Read Immediate: never pending, answered at once.
    Immediate,
    /// Read MDT Immediate Alternate (R-DS-7): never pending, answered at once.
    ImmediateAlternate,
}

/// A screen kept by Save Screen until the host restores it.
#[derive(Debug, Clone)]
struct SavedScreen {
    id: u16,
    screen: Screen,
    pending_read: Option<ReadKind>,
    locked: bool,
    error_row: Option<usize>,
}

/// At most this many saved screens are kept; the host restores the recent ones.
const MAX_SAVED_SCREENS: usize = 16;

#[derive(Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent indicators of the operator information area"
)]
pub struct Session {
    config: SessionConfig,
    telnet: Telnet,
    screen: Screen,
    locked: bool,
    error: Option<OperatorError>,
    insert: bool,
    pending_read: Option<ReadKind>,
    /// Where the cursor goes when the keyboard unlocks (Insert Cursor order).
    insert_cursor: Option<usize>,
    /// The 0-based row for error messages (Start of Header); the last row if `None`.
    error_row: Option<usize>,
    /// The row Write Error Code overwrote, and what was on it, for Reset to put back.
    saved_error_line: Option<(usize, Vec<u8>)>,
    saved_screens: Vec<SavedScreen>,
    next_save_id: u16,
    message_waiting: bool,
    alarm: bool,
    notice: Option<String>,
    outbound: Vec<u8>,
    /// The steps recorded so far, while recording a macro (R-KEY-3).
    recording: Option<Vec<Step>>,
    /// The macro being played (R-KEY-4).
    player: Option<playback::Player>,
    /// How often the host has unlocked the keyboard: a macro's `wait` step watches it.
    unlocks: u64,
    /// Type-ahead is on (R-KEY-11).
    type_ahead_on: bool,
    /// Keys kept while the keyboard waits for the host (R-KEY-11).
    typed_ahead: typeahead::TypedAhead,
    /// The host's programmable mouse buttons (R-DS-12), and the two-event one whose first
    /// event came.
    mouse_buttons: Vec<mouse::MouseDefinition>,
    mouse_first: Option<usize>,
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        let telnet = Telnet::new(config.terminal_type(), config.environment());
        Self {
            config,
            telnet,
            screen: Screen::new(ScreenSize::Standard),
            locked: true,
            error: None,
            insert: false,
            pending_read: None,
            insert_cursor: None,
            error_row: None,
            saved_error_line: None,
            saved_screens: Vec::new(),
            next_save_id: 0,
            message_waiting: false,
            alarm: false,
            notice: None,
            outbound: Vec::new(),
            recording: None,
            player: None,
            unlocks: 0,
            type_ahead_on: false,
            typed_ahead: typeahead::TypedAhead::new(),
            mouse_buttons: Vec::new(),
            mouse_first: None,
        }
    }

    pub const fn config(&self) -> &SessionConfig {
        &self.config
    }

    pub const fn code_page(&self) -> CodePage {
        self.config.code_page
    }

    pub const fn screen(&self) -> &Screen {
        &self.screen
    }

    pub const fn keyboard(&self) -> KeyboardState {
        match (self.error, self.locked) {
            (Some(error), _) => KeyboardState::Error(error),
            (None, true) => KeyboardState::Locked,
            (None, false) => KeyboardState::Unlocked,
        }
    }

    pub const fn insert_mode(&self) -> bool {
        self.insert
    }

    pub const fn message_waiting(&self) -> bool {
        self.message_waiting
    }

    /// True once after the host asked for the alarm to sound.
    pub fn take_alarm(&mut self) -> bool {
        std::mem::take(&mut self.alarm)
    }

    /// The last problem with the host's data, if any: the session goes on after one.
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Shows `text` as the session's notice, as for a problem with the host's data.
    pub(crate) fn set_notice(&mut self, text: String) {
        self.notice = Some(text);
    }

    /// Bytes from the host. Replies are queued for [`Session::take_outbound`].
    pub fn receive(&mut self, bytes: &[u8]) {
        let mut received = Received::default();
        self.telnet.receive(bytes, &mut received);
        self.outbound.extend(received.reply);
        if received.dropped > 0 {
            self.notice = Some(
                Error::data("a record longer than a 5250 record can be; it was dropped")
                    .to_string(),
            );
        }
        for bytes in received.records {
            if let Err(e) = self.process_record(&bytes) {
                self.notice = Some(e.to_string());
            }
        }
        self.replay_typed_ahead();
        self.advance_macro();
    }

    /// Queues a telnet No Operation, so an idle connection is not dropped (R-NET-5).
    pub fn keep_alive(&mut self) {
        self.outbound.extend(Telnet::KEEP_ALIVE);
    }

    /// Whether the field starting at `start` is non-display (its attribute is just before it).
    fn field_is_hidden(&self, start: usize) -> bool {
        Style::from_attribute(self.screen.get(self.screen.previous(start))).hidden
    }

    /// The bytes to send to the host, telnet-framed.
    pub fn take_outbound(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.outbound)
    }

    fn process_record(&mut self, bytes: &[u8]) -> Result<()> {
        let record = record::parse(bytes)?;
        match record.opcode {
            Opcode::SaveScreen => self.save_screen(),
            Opcode::CancelInvite => self.send_record(0, Opcode::CancelInvite, &[]),
            Opcode::MessageLightOn => self.message_waiting = true,
            Opcode::MessageLightOff => self.message_waiting = false,
            Opcode::NoOperation
            | Opcode::Invite
            | Opcode::OutputOnly
            | Opcode::PutGet
            | Opcode::RestoreScreen
            | Opcode::ReadImmediate
            | Opcode::ReadScreen => self.process_stream(record.data)?,
        }
        Ok(())
    }

    fn send_record(&mut self, flags: u8, opcode: Opcode, data: &[u8]) {
        self.outbound
            .extend(Telnet::frame(&record::build(flags, opcode, data)));
    }

    fn lock(&mut self) {
        self.locked = true;
    }

    /// Unlocks the keyboard; the cursor goes to the Insert Cursor position if the host set one,
    /// else (if the keyboard was locked) to the first input field.
    fn unlock(&mut self) {
        if let Some(address) = self.insert_cursor.take() {
            self.screen.cursor = address;
        } else if self.locked {
            self.screen.cursor = self.screen.home();
        }
        self.locked = false;
        self.unlocks = self.unlocks.wrapping_add(1);
        self.screen.reset_selections();
    }
}
