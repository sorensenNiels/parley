//! The start screen's lists (R-SSH-63..65): one line for each saved session or host, and headings that fold.

use egui::{Label, RichText, Sense, Ui};
use parley_ui_egui::{icons, theme};

/// The star in front of a row: none for what is not saved (R-SSH-63).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Star {
    None,
    Off,
    On,
}

/// One line of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub address: String,
    pub star: Star,
    /// The button beside Connect: "Edit…" or "Save as session…".
    pub second: &'static str,
    /// Chosen with Up and Down in the search's matches (R-SSH-65).
    pub selected: bool,
}

/// What was clicked in a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clicked {
    Connect,
    Second,
    Star,
}

/// The names of the star for screen readers, as it is now.
pub const ADD_FAVOURITE: &str = "Add to favourites";
pub const REMOVE_FAVOURITE: &str = "Remove from favourites";

/// One line: the star, the name, the address, `second` and Connect; scrolled into view when `scroll`.
pub fn row(ui: &mut Ui, row: &Row, scroll: bool) -> Option<Clicked> {
    let p = theme::palette(ui);
    let mut frame = theme::card(ui).inner_margin(egui::Margin::symmetric(8, 4));
    if row.selected {
        frame = frame.stroke(egui::Stroke::new(1.0, p.accent));
    }
    let shown = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let mut clicked = None;
            if star(ui, row.star) {
                clicked = Some(Clicked::Star);
            }
            theme::button_row(ui, |ui| {
                if ui.add(theme::primary_button(ui, "Connect")).clicked() {
                    clicked = Some(Clicked::Connect);
                }
                if ui.button(row.second).clicked() {
                    clicked = Some(Clicked::Second);
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    // The name gets most of the room; the address what is left.
                    let room = ui.available_width();
                    ui.scope(|ui| {
                        ui.set_max_width(room * 0.6);
                        ui.add(Label::new(theme::semibold(&row.name)).truncate());
                    });
                    let address = RichText::new(&row.address).small().color(p.muted);
                    ui.add(Label::new(address).truncate());
                });
            });
            clicked
        })
        .inner
    });
    if scroll && row.selected {
        shown.response.scroll_to_me(None);
    }
    shown.inner
}

/// The star of a saved session; true when it was clicked.
fn star(ui: &mut Ui, star: Star) -> bool {
    let on = match star {
        Star::None => return false,
        Star::Off => false,
        Star::On => true,
    };
    let p = theme::palette(ui);
    let (color, name) = if on {
        (p.accent, REMOVE_FAVOURITE)
    } else {
        (p.muted, ADD_FAVOURITE)
    };
    let button = theme::ghost_button(icons::icon(icons::STAR).color(color));
    icons::add_named(ui, true, button, name)
        .on_hover_text(name)
        .clicked()
}

/// Whether a section is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    /// Open at first when true; egui remembers it while Parley SSH runs.
    Starts(bool),
    /// Open when true, kept by the caller: a click is returned for it to keep.
    Kept(bool),
}

/// A heading with `count` that folds what is under it; what the body returned, and true when the heading or its
/// arrow was clicked.
pub fn section<R>(
    ui: &mut Ui,
    id: &str,
    label: &str,
    count: usize,
    fold: Fold,
    body: impl FnOnce(&mut Ui) -> R,
) -> (Option<R>, bool) {
    let id = ui.make_persistent_id(id);
    let (open, kept) = match fold {
        Fold::Starts(open) => (open, false),
        Fold::Kept(open) => (open, true),
    };
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, open);
    if kept {
        // As the caller keeps it, so a click flips that.
        state.set_open(open);
    }
    let mut clicked = false;
    let mut header = state.show_header(ui, |ui| {
        theme::section_label(ui, &format!("{label} ({count})"));
        // The heading folds too, not only the arrow before it.
        clicked = ui
            .interact(ui.min_rect(), id.with("heading"), Sense::click())
            .clicked();
    });
    if clicked {
        header.toggle();
    }
    let (arrow, _, shown) = header.body_unindented(body);
    (shown.map(|b| b.inner), clicked || arrow.clicked())
}
