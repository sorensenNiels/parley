//! The start screens' lists (R-SSH-63..65, R-UI-40..42, R-SQL-105; D64): one line for each saved session, host or
//! group, and headings that fold.

use crate::{icons, theme};
use egui::{Label, RichText, Sense, Ui};

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
    /// The main button: "Connect", or "Start" for a group (R-UI-42).
    pub primary: &'static str,
    /// The button beside it: "Edit…" or "Save as session…".
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

/// One line: the star, the name, the address, `second` and `primary`; scrolled into view when `scroll`.
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
                if ui.add(theme::primary_button(ui, row.primary)).clicked() {
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

/// Whether "All sessions" is open: always when nothing is listed above it, else as the user left it (R-SSH-65,
/// R-UI-42, R-SQL-105; D64).
pub const fn all_open(kept: bool, above: bool) -> bool {
    kept || !above
}

/// Whether a section is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    /// Open at first when true; egui remembers it while the program runs.
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

/// What the keys did in [`search_field`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the keys of one frame, each pressed or not"
)]
pub struct Search {
    /// The text changed: the first match is chosen again.
    pub changed: bool,
    pub down: bool,
    pub up: bool,
    /// Enter, to connect the chosen match.
    pub enter: bool,
    /// Escape: the text was emptied.
    pub escape: bool,
}

/// The search field over a start screen's list (R-SSH-65, R-UI-42, R-SQL-105): takes the keyboard when `focus` (then
/// clears it), passes Up and Down on, and empties itself on Escape.
pub fn search_field(
    ui: &mut Ui,
    id: &str,
    text: &mut String,
    hint: &str,
    focus: &mut bool,
) -> Search {
    let id = ui.make_persistent_id(id);
    // Taken before the field sees them: a one-line field has no use for them.
    let (down, up) = if ui.memory(|m| m.has_focus(id)) {
        ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            )
        })
    } else {
        (false, false)
    };
    let edit = egui::TextEdit::singleline(text).id(id).hint_text(hint);
    let response = theme::sized_edit(ui, ui.available_width(), edit);
    if std::mem::take(focus) {
        response.request_focus();
    }
    let mut search = Search {
        changed: response.changed(),
        down,
        up,
        ..Search::default()
    };
    if response.lost_focus() {
        let (enter, escape) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if escape {
            text.clear();
            response.request_focus();
        }
        search.enter = enter && !text.trim().is_empty();
        search.escape = escape;
    }
    search
}

/// The widths of a start screen's list and of what stays beside it, and the room between them.
const LIST_WIDTH: f32 = 560.0;
const SIDE_WIDTH: f32 = 400.0;
const GAP: f32 = 24.0;

/// A start screen's two columns, centred (R-SSH-65, R-UI-42): `list` with `side` beside it when there is room, `side`
/// above it when not; what either chose, the list's first.
pub fn columns<T, C>(
    ui: &mut Ui,
    state: &mut T,
    mut list: impl FnMut(&mut T, &mut Ui) -> Option<C>,
    mut side: impl FnMut(&mut T, &mut Ui) -> Option<C>,
) -> Option<C> {
    let both = LIST_WIDTH + GAP + SIDE_WIDTH;
    if ui.available_width() >= both + 32.0 {
        let margin = (ui.available_width() - both) / 2.0;
        ui.horizontal_top(|ui| {
            ui.add_space(margin);
            let listed = ui
                .vertical(|ui| {
                    ui.set_width(LIST_WIDTH);
                    list(state, ui)
                })
                .inner;
            ui.add_space(GAP);
            let beside = ui
                .vertical(|ui| {
                    ui.set_width(SIDE_WIDTH);
                    side(state, ui)
                })
                .inner;
            listed.or(beside)
        })
        .inner
    } else {
        let width = LIST_WIDTH.min(ui.available_width());
        let margin = (ui.available_width() - width) / 2.0;
        ui.horizontal_top(|ui| {
            ui.add_space(margin);
            ui.vertical(|ui| {
                ui.set_width(width);
                let above = side(state, ui);
                ui.add_space(20.0);
                list(state, ui).or(above)
            })
            .inner
        })
        .inner
    }
}
