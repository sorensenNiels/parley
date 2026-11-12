//! The status bar (R-UI-2): boxes along the bottom of the window, like the operator information
//! area of a 5250 display. From the left: the keyboard state or message, the connection,
//! TLS, insert mode, message waiting, and the cursor's row and column.

use egui::{Align, Frame, Layout, Margin, RichText, Stroke, Ui};
use parley_core::net::LinkState;
use parley_core::{KeyboardState, MacroState, Session};

use crate::theme;

/// `session` and `link` are `None` while there is no connection. `note` is why the last one
/// ended, or a warning about the open one. The macro box is there only with `macros` (the
/// Macros add-on, R-ADD-2). Returns true when its Stop button was clicked (R-KEY-6).
pub fn status_bar(
    ui: &mut Ui,
    session: Option<&Session>,
    link: Option<&LinkState>,
    tls: bool,
    note: Option<&str>,
    macros: bool,
) -> bool {
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
        let stop = macros && macro_segment(ui, session);
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
        stop
    })
    .inner
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

/// The macro box's width: room for "REC" and Stop, always, so the boxes beside it don't move
/// when a recording starts.
const MACRO_WIDTH: f32 = 104.0;

/// "REC" and a Stop button while recording a macro (R-KEY-3, R-KEY-6), "⏵ Macro" while one
/// runs (R-KEY-4). Returns true when Stop was clicked.
fn macro_segment(ui: &mut Ui, session: Option<&Session>) -> bool {
    let p = theme::palette(ui);
    let (text, hover) = match session.map(|s| (s.macro_state(), s.playing())) {
        Some((MacroState::Recording, _)) => return recording_segment(ui),
        Some((MacroState::Playing | MacroState::NeedsPassword, name)) => (
            RichText::new("⏵ Macro").color(p.ok),
            format!("Running \"{}\": Reset stops it", name.unwrap_or_default()),
        ),
        Some((MacroState::Off, _)) | None => (RichText::new(""), String::new()),
    };
    let response = segment(ui, MACRO_WIDTH, text);
    if !hover.is_empty() {
        response.on_hover_text(hover);
    }
    false
}

/// "REC" and a small Stop button, which ends the recording as Tools ▸ Stop recording does.
fn recording_segment(ui: &mut Ui) -> bool {
    let rec = RichText::new("REC").color(theme::palette(ui).fail).strong();
    framed(ui, MACRO_WIDTH, |ui| {
        ui.label(rec).on_hover_text("Recording a macro");
        let stop = egui::Button::new(RichText::new("Stop").small()).small();
        ui.add(stop)
            .on_hover_text("Stop recording and name the macro")
            .clicked()
    })
    .inner
}

/// One box of the status bar, `width` wide, its text on the left.
fn segment(ui: &mut Ui, width: f32, text: RichText) -> egui::Response {
    framed(ui, width, |ui| {
        ui.add(egui::Label::new(text).truncate());
    })
    .response
}

/// A box of the status bar, `width` wide, with `add` laid out from the left.
fn framed<R>(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    Frame::new()
        .stroke(Stroke::new(1.0, theme::palette(ui).border))
        .inner_margin(Margin::symmetric(8, 3))
        .corner_radius(3)
        .show(ui, |ui| {
            ui.set_width(width);
            ui.with_layout(Layout::left_to_right(Align::Center), add)
                .inner
        })
}
