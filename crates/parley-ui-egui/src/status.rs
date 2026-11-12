//! The status bar (R-UI-2): boxes along the bottom of the window, like the operator information
//! area of a 5250 display. From the left: the keyboard state or message, the connection,
//! TLS, insert mode, message waiting, and the cursor's row and column.

use egui::{Align, Frame, Layout, Margin, RichText, Stroke, Ui};
use parley_core::net::LinkState;
use parley_core::{KeyboardState, MacroState, Session};

use crate::theme;

/// `session` and `link` are `None` while there is no connection. `note` is why the last one
/// ended, or a warning about the open one.
pub fn status_bar(
    ui: &mut Ui,
    session: Option<&Session>,
    link: Option<&LinkState>,
    tls: bool,
    note: Option<&str>,
) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let cursor = session.map(|s| {
            let (row, column) = s.screen().row_column(s.screen().cursor);
            format!("{},{}", row + 1, column + 1)
        });
        segment(
            ui,
            72.0,
            RichText::new(cursor.unwrap_or_default()).monospace(),
        );
        let mw = session.is_some_and(Session::message_waiting);
        segment(
            ui,
            36.0,
            RichText::new(if mw { "MW" } else { "" })
                .color(theme::palette(ui).warning_text)
                .strong(),
        )
        .on_hover_text("Message waiting");
        macro_segment(ui, session);
        let insert = session.is_some_and(Session::insert_mode);
        segment(ui, 56.0, RichText::new(if insert { "Insert" } else { "" }));
        let secure = tls && link.is_some();
        segment(ui, 36.0, RichText::new(if secure { "TLS" } else { "" }));
        let connection = match link {
            None | Some(LinkState::Closed(_)) => {
                RichText::new("Offline").color(theme::palette(ui).muted)
            }
            Some(LinkState::Connecting) => RichText::new("Connecting…"),
            Some(LinkState::Connected) => RichText::new("Online"),
        };
        segment(ui, 110.0, connection);
        let message = message(ui, session, link, note);
        let rest = (ui.available_width() - 20.0).max(40.0);
        segment(ui, rest, message);
    });
}

/// The first box: what the keyboard is doing, or why the last connection ended.
fn message(
    ui: &Ui,
    session: Option<&Session>,
    link: Option<&LinkState>,
    note: Option<&str>,
) -> RichText {
    let p = theme::palette(ui);
    let Some(session) = session else {
        return RichText::new(note.unwrap_or("")).color(p.muted);
    };
    if matches!(link, Some(LinkState::Connecting)) {
        return RichText::new("");
    }
    let text = match session.keyboard() {
        KeyboardState::Unlocked => RichText::new("Ready").color(p.ok),
        KeyboardState::Locked => RichText::new("X SYSTEM").color(p.warning_text),
        KeyboardState::Error(error) => {
            RichText::new(format!("X  {}  Press Reset (Ctrl).", error.message())).color(p.fail)
        }
    };
    match session.notice().or(note) {
        Some(notice) if session.keyboard() == KeyboardState::Unlocked => {
            RichText::new(format!("Ready  ·  {notice}")).color(p.warning_text)
        }
        _ => text,
    }
}

/// "REC" while recording a macro, "⏵ Macro" while one runs (R-KEY-3, R-KEY-4).
fn macro_segment(ui: &mut Ui, session: Option<&Session>) {
    let p = theme::palette(ui);
    let (text, hover) = match session.map(|s| (s.macro_state(), s.playing())) {
        Some((MacroState::Recording, _)) => (
            RichText::new("REC").color(p.fail).strong(),
            "Recording a macro: Stop recording is in the Tools menu".to_owned(),
        ),
        Some((MacroState::Playing | MacroState::NeedsPassword, name)) => (
            RichText::new("⏵ Macro").color(p.ok),
            format!("Running \"{}\": Reset stops it", name.unwrap_or_default()),
        ),
        Some((MacroState::Off, _)) | None => (RichText::new(""), String::new()),
    };
    let response = segment(ui, 64.0, text);
    if !hover.is_empty() {
        response.on_hover_text(hover);
    }
}

/// One box of the status bar, `width` wide, its text on the left.
fn segment(ui: &mut Ui, width: f32, text: RichText) -> egui::Response {
    Frame::new()
        .stroke(Stroke::new(1.0, theme::palette(ui).border))
        .inner_margin(Margin::symmetric(8, 3))
        .corner_radius(3)
        .show(ui, |ui| {
            ui.set_width(width);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(text).truncate());
            });
        })
        .response
}
