//! The status bar (R-UI-14, replacing R-UI-2's boxes): a pill for the keyboard state on the
//! left, the Keys menu (R-UI-16), then the macro being recorded or run; on the right, each only while it applies,
//! message waiting, insert mode, TLS, the keyboard map (a menu), the code page, the screen's
//! size, the host's last response time and the cursor's row and column.

use std::time::{Duration, Instant};

use egui::{Align, Layout, RichText, Ui};
use parley_core::net::LinkState;
use parley_core::{KeyboardState, MacroState, Session, SessionConfig};

use crate::keymap::KeyMap;
use crate::{icons, keys, theme};

/// What the status bar shows besides the session.
#[derive(Debug)]
pub struct Status<'a> {
    /// `None` while there is no connection.
    pub session: Option<&'a Session>,
    pub link: Option<&'a LinkState>,
    /// The session in use, or the last one used.
    pub config: Option<&'a SessionConfig>,
    /// Why the last connection ended, or a warning about the open one.
    pub note: Option<&'a str>,
    /// The Macros add-on is on (R-ADD-2).
    pub macros: bool,
    /// The last session is a saved one, so it can be connected to again.
    pub can_connect_again: bool,
    /// The keyboard map in use and every map there is, Default first (R-KEY-2).
    pub map: &'a str,
    /// The keyboard map in use itself, for the keys the Keys menu shows.
    pub keymap: &'a KeyMap,
    /// Name keys as macOS does (Cmd+C).
    pub mac: bool,
    pub maps: &'a [String],
    pub clock: &'a Clock,
}

/// What was clicked in the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusAction {
    /// Stop beside REC (R-KEY-6).
    StopRecording,
    /// Connect again to the last session.
    ConnectAgain,
    /// Use this keyboard map for the session.
    UseMap(String),
    /// A 5250 key from the Keys menu (R-UI-16).
    Key(parley_core::Key),
}

/// The times the status bar shows, from the keyboard and macro state seen frame by frame.
#[derive(Debug, Default)]
pub struct Clock {
    /// When the keyboard locked, waiting for the host.
    locked_since: Option<Instant>,
    /// How long the host took the last time.
    response: Option<Duration>,
    recording_since: Option<Instant>,
}

impl Clock {
    /// Notes the state of `session` at `now`; `None` (no connection) forgets the times.
    pub fn observe(&mut self, session: Option<&Session>, now: Instant) {
        let Some(session) = session else {
            *self = Self::default();
            return;
        };
        let locked = session.keyboard() == KeyboardState::Locked;
        match (locked, self.locked_since) {
            (true, None) => self.locked_since = Some(now),
            (false, Some(since)) => {
                self.response = Some(now.saturating_duration_since(since));
                self.locked_since = None;
            }
            (true, Some(_)) | (false, None) => {}
        }
        let recording = session.macro_state() == MacroState::Recording;
        match (recording, self.recording_since) {
            (true, None) => self.recording_since = Some(now),
            (false, Some(_)) => self.recording_since = None,
            (true, Some(_)) | (false, None) => {}
        }
    }

    /// How long the host took to answer the last AID key.
    pub const fn response(&self) -> Option<Duration> {
        self.response
    }

    /// The keyboard waits for the host, as last observed.
    pub const fn waiting(&self) -> bool {
        self.locked_since.is_some()
    }

    /// How long the macro has been recorded, at `now`.
    pub fn recording(&self, now: Instant) -> Option<Duration> {
        self.recording_since
            .map(|since| now.saturating_duration_since(since))
    }
}

/// The Ready icon: always there while the keyboard takes input or waits for the host; named
/// "Ready" for screen readers only while it takes input.
fn ready_icon(ui: &mut Ui, ready: bool) {
    let p = theme::palette(ui);
    let text = icons::icon(icons::CHECK_CIRCLE)
        .size(STATE_ICON_SIZE)
        .color(p.ok);
    let response = ui.add(egui::Label::new(text).selectable(false));
    let name = if ready { "Ready" } else { "Connected" };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, name));
    response.on_hover_text(if ready {
        "Ready for input"
    } else {
        "Connected; the host is processing"
    });
}

/// The processing slot (R-UI-14): an hourglass while the host works ("X SYSTEM"), empty
/// otherwise but always as wide, so nothing beside it moves.
fn processing_slot(ui: &mut Ui, processing: bool) {
    let size = egui::vec2(STATE_ICON_SIZE + 4.0, ui.spacing().interact_size.y);
    if processing {
        let color = theme::palette(ui).warning_text;
        let text = icons::icon(icons::HOURGLASS)
            .size(STATE_ICON_SIZE)
            .color(color);
        let response = ui.add_sized(size, egui::Label::new(text).selectable(false));
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, "X SYSTEM"));
        response.on_hover_text("The host is processing (X SYSTEM)");
    } else {
        ui.add_sized(size, egui::Label::new("").selectable(false));
    }
}

/// The state icon's size, a little larger than text.
const STATE_ICON_SIZE: f32 = 17.0;

/// Room between two chips on the right.
const CHIP_GAP: f32 = 14.0;

/// Room kept for the left side (the state pill and the note) when the right side's chips are
/// chosen.
const LEFT_ROOM: f32 = 260.0;

pub fn status_bar(ui: &mut Ui, status: &Status<'_>) -> Option<StatusAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        action = left(ui, status);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = CHIP_GAP;
            if let Some(chosen) = right(ui, status) {
                action = Some(chosen);
            }
        });
    });
    action
}

/// The state pill, the macro and the note; Stop and Connect again.
fn left(ui: &mut Ui, status: &Status<'_>) -> Option<StatusAction> {
    let p = theme::palette(ui);
    let mut action = None;
    let Some(session) = status.session else {
        theme::badge(ui, "Offline", p.muted);
        if let Some(note) = status.note {
            ui.add(egui::Label::new(RichText::new(note).color(p.muted)).truncate());
        }
        let again =
            icons::button(icons::ARROWS_CLOCKWISE, "Connect again").frame_when_inactive(false);
        if status.can_connect_again
            && icons::add_named(ui, true, again.small(), "Connect again").clicked()
        {
            action = Some(StatusAction::ConnectAgain);
        }
        return action;
    };
    if matches!(status.link, Some(LinkState::Connecting)) {
        theme::badge(ui, "Connecting…", p.muted);
        return None;
    }
    match session.keyboard() {
        KeyboardState::Unlocked => ready_icon(ui, true),
        KeyboardState::Locked => ready_icon(ui, false),
        KeyboardState::Error(error) => {
            theme::badge(ui, format!("X {}", error.message()), p.fail);
            ui.label(RichText::new("Press Reset").color(p.muted));
        }
    }
    processing_slot(ui, session.keyboard() == KeyboardState::Locked);
    if let Some(key) = keys::keys_menu(ui, status.keymap, status.mac) {
        action = Some(StatusAction::Key(key));
    }
    if status.macros && macro_pill(ui, session, status.clock) {
        action = Some(StatusAction::StopRecording);
    }
    if let Some(notice) = session.notice().or(status.note) {
        let text = RichText::new(notice).color(p.warning_text);
        ui.add(egui::Label::new(text).truncate());
    }
    action
}

/// "REC m:ss" and Stop while recording (R-KEY-3, R-KEY-6), the macro's name while one runs
/// (R-KEY-4). True when Stop was clicked.
fn macro_pill(ui: &mut Ui, session: &Session, clock: &Clock) -> bool {
    let p = theme::palette(ui);
    match session.macro_state() {
        MacroState::Recording => {
            let elapsed = clock.recording(Instant::now()).unwrap_or_default();
            let secs = elapsed.as_secs();
            let text = format!("REC {}:{:02}", secs / 60, secs % 60);
            theme::icon_badge(ui, icons::RECORD, text, p.fail).on_hover_text("Recording a macro");
            ui.ctx().request_repaint_after(Duration::from_secs(1));
            let stop = icons::button(icons::STOP, "Stop").small();
            icons::add_named(ui, true, stop, "Stop")
                .on_hover_text("Stop recording and name the macro")
                .clicked()
        }
        MacroState::Playing | MacroState::NeedsPassword => {
            let name = session.playing().unwrap_or_default();
            theme::icon_badge(ui, icons::PLAY, name, p.ok)
                .on_hover_text("Running a macro: Reset stops it");
            false
        }
        MacroState::Off => false,
    }
}

/// One chip on the right side.
#[derive(Debug)]
enum Chip {
    Cursor(String),
    Insert,
    MessageWaiting,
    Tls,
    Map,
    CodePage(String),
    Size(String),
    Response(String),
}

/// The chips that apply, from the right edge leftwards, then those that fit.
fn right(ui: &mut Ui, status: &Status<'_>) -> Option<StatusAction> {
    let mut chips = Vec::new();
    if let Some(session) = status.session {
        let screen = session.screen();
        let (row, column) = screen.row_column(screen.cursor);
        chips.push(Chip::Cursor(format!("{}, {}", row + 1, column + 1)));
        if session.insert_mode() {
            chips.push(Chip::Insert);
        }
        if session.message_waiting() {
            chips.push(Chip::MessageWaiting);
        }
        if status.config.is_some_and(|c| c.tls) {
            chips.push(Chip::Tls);
        }
        chips.push(Chip::Map);
        if let Some(config) = status.config {
            chips.push(Chip::CodePage(format!(
                "CCSID {}",
                config.code_page.ccsid()
            )));
        }
        let size = screen.size();
        chips.push(Chip::Size(format!("{}×{}", size.rows(), size.columns())));
        if let Some(time) = status.clock.response() {
            chips.push(Chip::Response(response_text(time)));
        }
    }
    let budget = ui.available_width() - LEFT_ROOM;
    while chips.len() > 1 && chips.iter().map(|c| chip_width(ui, c, status)).sum::<f32>() > budget {
        // The last ones are the first to go: response time, size, code page, map.
        let Some(at) = chips.iter().rposition(Chip::can_hide) else {
            break;
        };
        chips.remove(at);
    }
    let mut action = None;
    for chip in &chips {
        if let Some(chosen) = show_chip(ui, chip, status) {
            action = Some(chosen);
        }
    }
    action
}

impl Chip {
    const fn can_hide(&self) -> bool {
        matches!(
            self,
            Self::Map | Self::CodePage(_) | Self::Size(_) | Self::Response(_)
        )
    }

    /// The chip's icon, if it has one, and its text.
    fn text(&self, status: &Status<'_>) -> (Option<&'static str>, String) {
        match self {
            Self::Cursor(text) | Self::CodePage(text) | Self::Size(text) => (None, text.clone()),
            Self::Insert => (None, "INS".to_owned()),
            Self::MessageWaiting => (Some(icons::ENVELOPE), "Message waiting".to_owned()),
            Self::Tls => (Some(icons::LOCK), "TLS".to_owned()),
            Self::Map => (Some(icons::KEYBOARD), status.map.to_owned()),
            Self::Response(text) => (Some(icons::LIGHTNING), text.clone()),
        }
    }
}

/// Room for an icon before a chip's text.
const ICON_ROOM: f32 = 18.0;

/// About how wide `chip` is drawn: its text, its icon and the room around it.
fn chip_width(ui: &Ui, chip: &Chip, status: &Status<'_>) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let (icon, text) = chip.text(status);
    let galley = ui
        .painter()
        .layout_no_wrap(text, font, egui::Color32::PLACEHOLDER);
    galley.size().x + CHIP_GAP + 8.0 + if icon.is_some() { ICON_ROOM } else { 0.0 }
}

/// A muted chip: its icon, then its text.
pub fn muted(ui: &mut Ui, icon: Option<&str>, text: String) -> egui::Response {
    let color = theme::palette(ui).muted;
    match icon {
        Some(icon) => icons::label(
            ui,
            icons::icon(icon).color(color),
            RichText::new(text).color(color),
        ),
        None => ui.label(RichText::new(text).color(color)),
    }
}

fn show_chip(ui: &mut Ui, chip: &Chip, status: &Status<'_>) -> Option<StatusAction> {
    let p = theme::palette(ui);
    let (icon, text) = chip.text(status);
    match chip {
        Chip::Cursor(_) => {
            ui.label(RichText::new(text).monospace().color(p.muted))
                .on_hover_text("Cursor row, column");
        }
        Chip::Insert => {
            ui.label(theme::semibold(text)).on_hover_text("Insert mode");
        }
        Chip::MessageWaiting => {
            theme::icon_badge(ui, icons::ENVELOPE, text, p.warning_text);
        }
        Chip::Tls => {
            muted(ui, icon, text).on_hover_text(tls_hover(status.config));
        }
        Chip::Map => return map_menu(ui, text, status),
        Chip::CodePage(_) => {
            let name = status.config.map_or("", |c| c.code_page.label());
            muted(ui, icon, text).on_hover_text(format!("Code page {name}"));
        }
        Chip::Size(_) => {
            muted(ui, icon, text).on_hover_text("Screen size: rows × columns");
        }
        Chip::Response(_) => {
            muted(ui, icon, text).on_hover_text("How long the host took to answer the last key");
        }
    }
    None
}

/// The keyboard map chip: a menu of the maps, the one in use ticked.
fn map_menu(ui: &mut Ui, text: String, status: &Status<'_>) -> Option<StatusAction> {
    let mut chosen = None;
    let muted = theme::palette(ui).muted;
    let atoms = (
        icons::icon(icons::KEYBOARD).color(muted),
        RichText::new(text).color(muted),
    );
    ui.menu_button(atoms, |ui| {
        theme::section_label(ui, "Keyboard map");
        for name in status.maps {
            let button = egui::Button::new(name.as_str()).selected(name == status.map);
            if ui.add(button).clicked() {
                chosen = Some(StatusAction::UseMap(name.clone()));
            }
        }
    })
    .response
    .on_hover_text("Keyboard map: click to change it for this session");
    chosen
}

fn tls_hover(config: Option<&SessionConfig>) -> String {
    let base = "Encrypted with TLS. The host's certificate is checked against the system's \
                trusted certificates";
    match config.map(|c| c.ca_file.as_str()) {
        Some(file) if !file.is_empty() => format!("{base} and {file}."),
        _ => format!("{base}."),
    }
}

/// "142 ms" under a second, "1.4 s" above.
fn response_text(time: Duration) -> String {
    let millis = time.as_millis();
    if millis < 1000 {
        format!("{millis} ms")
    } else {
        format!("{:.1} s", time.as_secs_f32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_time_reads_in_milliseconds_under_a_second_r_ui_14() {
        assert_eq!(response_text(Duration::from_millis(142)), "142 ms");
        assert_eq!(response_text(Duration::from_millis(999)), "999 ms");
        assert_eq!(response_text(Duration::from_millis(1400)), "1.4 s");
        assert_eq!(response_text(Duration::ZERO), "0 ms");
    }

    #[test]
    fn the_clock_times_the_host_from_an_aid_key_to_the_unlocked_keyboard_r_ui_14() {
        use parley_core::{Aid, Key};
        let mut session = crate::test_support::waiting_session();
        let start = Instant::now();
        let mut clock = Clock::default();
        clock.observe(Some(&session), start);
        assert_eq!(clock.response(), None, "nothing asked yet");
        session.key(Key::Aid(Aid::ENTER));
        clock.observe(Some(&session), start);
        session.receive(&crate::test_support::waiting_screen());
        clock.observe(Some(&session), start + Duration::from_millis(142));
        assert_eq!(clock.response(), Some(Duration::from_millis(142)));
    }

    #[test]
    fn the_clock_forgets_its_times_without_a_connection_r_ui_14() {
        let mut clock = Clock {
            locked_since: None,
            response: Some(Duration::from_millis(5)),
            recording_since: Some(Instant::now()),
        };
        clock.observe(None, Instant::now());
        assert_eq!(clock.response(), None);
        assert_eq!(clock.recording(Instant::now()), None);
    }

    #[test]
    fn tls_hover_names_the_extra_ca_file_r_ui_14() {
        let mut config = SessionConfig::default();
        assert!(tls_hover(Some(&config)).ends_with("trusted certificates."));
        config.ca_file = "/etc/ca/company.pem".to_owned();
        assert!(tls_hover(Some(&config)).ends_with("and /etc/ca/company.pem."));
    }
}
