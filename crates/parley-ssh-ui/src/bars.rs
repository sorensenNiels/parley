//! Parley SSH's header and status bar (R-SSH-10; the session in front, R-SSH-39), in the look of Parley 5250 and Parley SQL: the icon and name,
//! the Session, Edit, View and Help menus; on the right the gear, the session pill and a newer version. Below,
//! where the session stands, its size, and on hover the algorithms in use and the host key. They only report what
//! the user chose, as an [`Action`].

use egui::{MenuBar, Ui};
use parley_ui_egui::menu::{self, Program};
use parley_ui_egui::{icons, status, theme};

use crate::session::{self, Session, State};

/// Something the user chose from the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A new tab with the start screen (R-SSH-39, R-SSH-41).
    NewTab,
    /// Another `parley-ssh` window (R-SSH-41, was R-SSH-15).
    NewWindow,
    /// The session in front again, in a new tab.
    Duplicate,
    /// A new pane beside or below the one in front (R-SSH-40).
    SplitRight,
    SplitDown,
    Restart,
    Disconnect,
    /// Close the pane in front, or its tab when it is the only pane.
    Close,
    /// The event log (R-SSH-16).
    EventLog,
    Exit,
    Copy,
    Paste,
    CopyAll,
    ClearScrollback,
    ResetTerminal,
    TextLarger,
    TextSmaller,
    TextActualSize,
    Shortcuts,
    Settings,
}

/// What the header shows.
pub struct Header<'a> {
    pub logo: Option<&'a egui::TextureHandle>,
    pub session: Option<&'a Session>,
    pub newer: Option<(&'a str, &'a str)>,
}

impl std::fmt::Debug for Header<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header")
            .field("session", &self.session.map(|s| s.target.label()))
            .finish_non_exhaustive()
    }
}

pub fn menu_bar(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let mut action = None;
    MenuBar::new().ui(ui, |ui| {
        menu::brand(ui, header.logo, Program::ParleySsh);
        let open = header.session.is_some();
        let connected = header.session.is_some_and(Session::is_connected);
        let session = [
            Item(true, icons::PLUS, "New tab", Action::NewTab),
            Item(true, icons::SQUARES_FOUR, "New window", Action::NewWindow),
            Item(open, icons::COPY, "Duplicate session", Action::Duplicate),
            Item(true, icons::SQUARES_FOUR, "Split right", Action::SplitRight),
            Item(true, icons::SQUARES_FOUR, "Split down", Action::SplitDown),
            Item(
                open,
                icons::ARROWS_CLOCKWISE,
                "Restart session",
                Action::Restart,
            ),
            Item(connected, icons::POWER, "Disconnect", Action::Disconnect),
            Item(true, icons::SIGN_OUT, "Close", Action::Close),
            Item(open, icons::LIST, "Event log", Action::EventLog),
            Item(true, icons::SIGN_OUT, "Exit", Action::Exit),
        ];
        let edit = [
            Item(open, icons::COPY, "Copy", Action::Copy),
            Item(connected, icons::CLIPBOARD, "Paste", Action::Paste),
            Item(open, icons::COPY, "Copy all", Action::CopyAll),
            Item(
                open,
                icons::ERASER,
                "Clear scrollback",
                Action::ClearScrollback,
            ),
            Item(open, icons::RESET, "Reset terminal", Action::ResetTerminal),
        ];
        let view = [
            Item(true, icons::PLUS, "Larger text", Action::TextLarger),
            Item(true, icons::LIST, "Smaller text", Action::TextSmaller),
            Item(true, icons::RESET, "Actual size", Action::TextActualSize),
        ];
        let help = [Item(
            true,
            icons::KEYBOARD,
            "Keyboard shortcuts",
            Action::Shortcuts,
        )];
        for (name, items) in [
            ("Session", &session[..]),
            ("Edit", &edit),
            ("View", &view),
            ("Help", &help),
        ] {
            ui.menu_button(name, |ui| {
                for item in items {
                    pick(ui, &mut action, item);
                }
            });
        }
        theme::button_row(ui, |ui| right_side(ui, header, &mut action));
    });
    action
}

/// The gear, the session pill and a newer version, laid out right to left.
fn right_side(ui: &mut Ui, header: &Header<'_>, action: &mut Option<Action>) {
    if menu::gear(ui) {
        *action = Some(Action::Settings);
    }
    if let Some(session) = header.session
        && let Some(chosen) = pill(ui, session)
    {
        *action = Some(chosen);
    }
    if let Some((version, page)) = header.newer {
        ui.hyperlink_to(format!("Parley {version} is available"), page)
            .on_hover_text("Opens the release page, with its notes and installers.");
        ui.add_space(8.0);
    }
}

/// A menu item: enabled, its icon and text, and what it does.
struct Item(bool, &'static str, &'static str, Action);

fn pick(ui: &mut Ui, action: &mut Option<Action>, item: &Item) {
    let Item(enabled, icon, text, chosen) = *item;
    if icons::add_named(ui, enabled, icons::button(icon, text), text).clicked() {
        *action = Some(chosen);
    }
}

/// The session in use (as Parley's session pill, R-UI-15): `user@host` with a menu to restart or disconnect it,
/// and a dot for its state. Laid out right to left.
fn pill(ui: &mut Ui, session: &Session) -> Option<Action> {
    let p = theme::palette(ui);
    let atoms = (
        theme::semibold(session.target.label()),
        icons::icon(icons::CARET_DOWN).color(p.muted),
    );
    let mut action = None;
    let response = ui.menu_button(atoms, |ui| {
        theme::section_label(ui, &session.target.label());
        let restart = Item(
            true,
            icons::ARROWS_CLOCKWISE,
            "Restart session",
            Action::Restart,
        );
        pick(ui, &mut action, &restart);
        let disconnect = Item(
            session.is_connected(),
            icons::POWER,
            "Disconnect",
            Action::Disconnect,
        );
        pick(ui, &mut action, &disconnect);
    });
    let (color, word) = match session.state {
        State::Running => (p.ok, "connected"),
        State::Connecting | State::SigningOn => (p.warning_text, "connecting"),
        State::Ended(_) => (p.muted, "not connected"),
    };
    response
        .response
        .on_hover_text(format!("{} is {word}", session.target.label()));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
    ui.add_space(4.0);
    action
}

/// The status bar: where the session stands; its size and security on the right.
pub fn status_bar(ui: &mut Ui, session: Option<&Session>, note: Option<&str>) {
    ui.horizontal(|ui| {
        let Some(session) = session else {
            status::muted(ui, None, note.unwrap_or("Not connected.").to_owned());
            return;
        };
        let text = match &session.state {
            State::Connecting => format!("Connecting to {}…", session.target.label()),
            State::SigningOn => format!("Signing on to {}…", session.target.label()),
            State::Running => format!("Connected to {}", session.target.label()),
            State::Ended(ending) => session::ending_text(ending),
        };
        status::muted(ui, Some(icons::TERMINAL), text);
        if let Some(problem) = &session.problem {
            ui.add_space(8.0);
            theme::warning(ui, problem);
        }
        theme::button_row(ui, |ui| {
            let (columns, rows) = session.size();
            status::muted(ui, None, format!("{columns}×{rows}"));
            if let Some(algorithms) = &session.algorithms {
                let fingerprint = session.fingerprint.as_deref().unwrap_or("");
                let about = format!(
                    "Key exchange: {}\nHost key: {} {}\nCipher: {}\nMAC: {}",
                    algorithms.kex,
                    algorithms.host_key,
                    fingerprint,
                    algorithms.cipher,
                    algorithms.mac
                );
                status::muted(ui, Some(icons::LOCK), "SSH-2".to_owned()).on_hover_text(about);
            }
        });
    });
}
