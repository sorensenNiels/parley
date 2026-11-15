//! Parley SQL's header and status bar (R-SQL-45), in Parley's look (R-UI-15): the icon and
//! name and File (no Help menu: the version and updates are on Settings ▸ About, R-SQL-86);
//! on the right a gear for Settings, the connection in use and a newer version. They only report what the operator chose, as an [`Action`].

use egui::{MenuBar, RichText, Sense, Ui};

use super::connections::SqlConnection;
use crate::menu::{self, Program};
use crate::sql::{Phase, WindowStatus};
use crate::{icons, status, theme};

/// Something the operator chose from the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewConnection,
    /// The connection dialog, on the connection in use or the first one.
    EditConnections,
    /// Connect to the saved connection at this index.
    Connect(usize),
    Disconnect,
    Exit,
    Settings,
}

/// What the header shows.
pub struct Header<'a> {
    pub logo: Option<&'a egui::TextureHandle>,
    pub connections: &'a [SqlConnection],
    /// The connection the SQL window is open on, and where the window is.
    pub current: Option<(&'a SqlConnection, Phase)>,
    pub newer: Option<&'a parley_core::update::Release>,
}

impl std::fmt::Debug for Header<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header")
            .field("current", &self.current.map(|(c, phase)| (&c.name, phase)))
            .finish_non_exhaustive()
    }
}

/// The header: brand and File; the gear, the connection pill and a newer version on
/// the right.
pub fn menu_bar(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let mut action = None;
    MenuBar::new().ui(ui, |ui| {
        menu::brand(ui, header.logo, Program::ParleySql);
        if let Some(chosen) = file_menu(ui, header) {
            action = Some(chosen);
        }
        theme::button_row(ui, |ui| {
            if menu::gear(ui) {
                action = Some(Action::Settings);
            }
            if let Some(chosen) = connection_pill(ui, header) {
                action = Some(chosen);
            }
            if let Some(release) = header.newer {
                ui.hyperlink_to(
                    format!("Parley {} is available", release.version),
                    &release.page,
                )
                .on_hover_text("Opens the release page, with its notes and installers.");
                ui.add_space(8.0);
            }
        });
    });
    action
}

/// One menu item with an icon; true when clicked.
fn item(ui: &mut Ui, enabled: bool, icon: &str, text: &str) -> bool {
    icons::add_named(ui, enabled, icons::button(icon, text), text).clicked()
}

fn file_menu(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let mut action = None;
    let connected = header.current.is_some();
    ui.menu_button("File", |ui| {
        if item(ui, true, icons::PLUS, "New connection…") {
            action = Some(Action::NewConnection);
        }
        let any = !header.connections.is_empty();
        if item(ui, any, icons::PENCIL, "Edit connections…") {
            action = Some(Action::EditConnections);
        }
        let names = header.connections.iter().map(|c| c.name.as_str());
        if let Some(i) = menu::connect_menu(ui, !connected && any, names, None) {
            action = Some(Action::Connect(i));
        }
        ui.separator();
        if item(ui, connected, icons::POWER, "Disconnect") {
            action = Some(Action::Disconnect);
        }
        ui.separator();
        if item(ui, true, icons::SIGN_OUT, "Exit") {
            action = Some(Action::Exit);
        }
    });
    action
}

/// The connection in use (as Parley's session pill, R-UI-15): a dot for where the window is,
/// its name, host and a lock for TLS; a menu to edit it or disconnect. Laid out right to left.
fn connection_pill(ui: &mut Ui, header: &Header<'_>) -> Option<Action> {
    let (connection, phase) = header.current?;
    let p = theme::palette(ui);
    let lock = if connection.tls { icons::LOCK } else { "" };
    let atoms = (
        theme::semibold(&connection.name),
        RichText::new(&connection.host).color(p.muted),
        icons::icon(lock).color(p.muted),
        icons::icon(icons::CARET_DOWN).color(p.muted),
    );
    let mut action = None;
    let response = ui.menu_button(atoms, |ui| {
        theme::section_label(ui, &connection.name);
        if item(ui, true, icons::PENCIL, "Edit connection…") {
            action = Some(Action::EditConnections);
        }
        ui.separator();
        if item(ui, true, icons::POWER, "Disconnect") {
            action = Some(Action::Disconnect);
        }
    });
    response
        .response
        .on_hover_text(format!("{} is {}", connection.name, phase_word(phase)));
    let color = match phase {
        Phase::Ready | Phase::Busy => p.ok,
        Phase::Connecting => p.warning_text,
        Phase::SignIn => p.muted,
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
    ui.add_space(4.0);
    action
}

const fn phase_word(phase: Phase) -> &'static str {
    match phase {
        Phase::SignIn => "not signed on",
        Phase::Connecting => "signing on",
        Phase::Ready => "ready",
        Phase::Busy => "running",
    }
}

/// What the status bar shows.
#[derive(Debug)]
pub struct Footer<'a> {
    /// The connection the window is open on and what the window says; `None` when closed.
    pub window: Option<(&'a SqlConnection, WindowStatus)>,
    /// A note while no window is open: why the last one closed.
    pub note: Option<&'a str>,
}

/// The status bar: where the window is on the left; TLS, autocommit, the job and the code
/// page on the right.
pub fn status_bar(ui: &mut Ui, footer: &Footer<'_>) {
    let p = theme::palette(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(22.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let Some((connection, window)) = &footer.window else {
            theme::badge(ui, "Offline", p.muted);
            if let Some(note) = footer.note {
                ui.add(egui::Label::new(RichText::new(note).color(p.muted)).truncate());
            }
            return;
        };
        left(ui, window);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            right(ui, connection, window);
        });
    });
}

/// Where the window is, and its last error.
fn left(ui: &mut Ui, window: &WindowStatus) {
    let p = theme::palette(ui);
    match window.phase {
        Phase::SignIn => {
            theme::badge(ui, "Not signed on", p.muted);
        }
        Phase::Connecting => {
            theme::badge(ui, "Signing on…", p.muted);
        }
        Phase::Ready => {
            theme::icon_badge(ui, icons::CHECK_CIRCLE, "Ready", p.ok);
        }
        Phase::Busy => {
            ui.spinner();
            ui.label(RichText::new("Running…").color(p.muted));
        }
    }
    if let Some(error) = &window.error {
        let text = RichText::new(error).color(p.warning_text);
        ui.add(egui::Label::new(text).truncate());
    }
}

/// From the right edge leftwards: code page, TLS, autocommit, the job.
fn right(ui: &mut Ui, connection: &SqlConnection, window: &WindowStatus) {
    let p = theme::palette(ui);
    let ccsid = format!("CCSID {}", connection.code_page.ccsid());
    status::muted(ui, None, ccsid).on_hover_text("For binary columns shown as text");
    if connection.tls {
        status::muted(ui, Some(icons::LOCK), "TLS".to_owned());
    } else {
        theme::badge(ui, "Not encrypted", p.warning_text)
            .on_hover_text("Statements and results travel as plain text.");
    }
    let Some(connected) = &window.connected else {
        return;
    };
    if window.uncommitted {
        theme::badge(ui, "Uncommitted changes", p.warning_text);
    } else if !window.autocommit {
        status::muted(ui, None, "Autocommit off".to_owned());
    }
    status::muted(ui, None, format!("job {}", connected.job))
        .on_hover_text(format!("{} · IBM i {}", connected.user, connected.release));
}
