//! The side panel on the left (R-SQL-82, D33, D34): Explorer (R-SQL-83) above Snippets (R-SQL-84),
//! each section folding on its own, a divider between them while both are open. The panel's
//! edge can be dragged to change its width, and the panel folds to a narrow strip.

use egui::{Panel, PointerButton, Rect, Ui, UiBuilder};
use serde::{Deserialize, Serialize};

use parley_sql_core::services;

use super::browser;
use super::layout::{self, Axis, Drag};
use super::{Around, SqlWindow};
use parley_ui_egui::{icons, theme};

/// The panel's width until it is dragged, and its least width (R-SQL-82).
pub const DEFAULT_WIDTH: u16 = 280;
const MIN_WIDTH: f32 = 200.0;
/// The folded strip's width.
const FOLDED_WIDTH: f32 = 36.0;
/// How near the panel's edge a double-click sets the usual width.
const EDGE: f32 = 5.0;
/// A section's header row.
const HEADER: f32 = 28.0;
/// The least height of a section's content while both are open (R-SQL-84).
const MIN_SECTION: f32 = 120.0;
/// Libraries' share of the height until the divider is dragged, in percent.
const DEFAULT_SHARE: u8 = 60;

/// How the side panel is laid out, kept in the settings between runs (R-SQL-82, R-SQL-84).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is a fold switch of its own, kept by name in the settings"
)]
pub struct SideLayout {
    /// The panel's width, in points.
    pub width: u16,
    /// Folded to a narrow strip.
    pub folded: bool,
    pub libraries_open: bool,
    pub snippets_open: bool,
    /// Libraries' share of the height while both sections are open, in percent.
    pub libraries_share: u8,
    /// Whether the group "Library list" is open (R-SQL-83).
    pub library_list_open: bool,
    /// Whether the System node is open (R-SQL-78).
    pub system_open: bool,
}

impl Default for SideLayout {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            folded: false,
            libraries_open: true,
            snippets_open: true,
            libraries_share: DEFAULT_SHARE,
            library_list_open: true,
            system_open: false,
        }
    }
}

/// A section of the side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Explorer,
    Snippets,
}

impl Section {
    /// The header's name: "Explorer" holds the IBM i, its System node and the libraries
    /// (R-SQL-87, D34).
    const fn name(self) -> &'static str {
        match self {
            Self::Explorer => "Explorer",
            Self::Snippets => "Snippets",
        }
    }
}

/// The heights of Libraries' and Snippets' content in `room`, the height left after the
/// headers and the divider: shared by `layout.libraries_share` while both are open, each
/// keeping its least height where there is room for both; all of it for the one open.
fn section_heights(layout: SideLayout, room: f32) -> (f32, f32) {
    let room = room.max(0.0);
    match (layout.libraries_open, layout.snippets_open) {
        (true, true) => {
            let share = f32::from(layout.libraries_share.min(100)) / 100.0;
            let libraries = if room >= 2.0 * MIN_SECTION {
                (room * share).clamp(MIN_SECTION, room - MIN_SECTION)
            } else {
                room * share
            };
            (libraries, room - libraries)
        }
        (true, false) => (room, 0.0),
        (false, true) => (0.0, room),
        (false, false) => (0.0, 0.0),
    }
}

/// `points` as a share in percent of `room`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0..=100 first"
)]
fn percent(points: f32, room: f32) -> u8 {
    if room <= 0.0 {
        return DEFAULT_SHARE;
    }
    (points / room * 100.0).round().clamp(0.0, 100.0) as u8
}

/// A width in points as the settings keep it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0..=u16::MAX first"
)]
fn whole_points(width: f32) -> u16 {
    width.round().clamp(0.0, f32::from(u16::MAX)) as u16
}

/// A small icon button named `name`; true when clicked.
fn panel_button(ui: &mut Ui, icon: &str, name: &str) -> bool {
    let glyph = icons::icon(icon).color(theme::palette(ui).muted);
    icons::add_named(ui, true, theme::ghost_button(glyph), name)
        .on_hover_text(name)
        .clicked()
}

impl SqlWindow {
    /// The side panel, or its folded strip (R-SQL-82).
    pub(super) fn side_panel(&mut self, ui: &mut Ui, around: Around<'_>) {
        if self.side.folded {
            Panel::left("sql-side-folded")
                .resizable(false)
                .exact_size(FOLDED_WIDTH)
                .show(ui, |ui| self.folded_side(ui));
            return;
        }
        let most = (ui.available_width() / 2.0).max(MIN_WIDTH);
        let mut panel = Panel::left("sql-side")
            .resizable(true)
            .size_range(MIN_WIDTH..=most)
            .default_size(f32::from(self.side.width));
        // The width kept, or the usual one after a double-click, is set once; then the panel
        // keeps what it is dragged to.
        if let Some(width) = self.side_width.take() {
            panel = panel.exact_size(f32::from(width).clamp(MIN_WIDTH, most));
        }
        let rect = panel.show(ui, |ui| self.side(ui, around)).response.rect;
        let on_edge = ui.input(|i| {
            i.pointer.button_double_clicked(PointerButton::Primary)
                && i.pointer.interact_pos().is_some_and(|p| {
                    (p.x - rect.right()).abs() <= EDGE && rect.y_range().contains(p.y)
                })
        });
        if on_edge {
            self.side_width = Some(DEFAULT_WIDTH);
        }
        self.side.width = whole_points(rect.width());
    }

    /// Folds or opens the panel (R-SQL-82: its buttons and Ctrl+B).
    pub(super) const fn toggle_side_panel(&mut self) {
        self.side.folded = !self.side.folded;
    }

    /// Opens the panel at Snippets, for a snippet just saved (R-SQL-84).
    pub(super) const fn show_snippets(&mut self) {
        self.side.folded = false;
        self.side.snippets_open = true;
    }

    /// The folded panel: a button that opens it again.
    fn folded_side(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            if panel_button(ui, icons::CARET_RIGHT, "Show Explorer and Snippets") {
                self.toggle_side_panel();
            }
        });
    }

    /// Libraries above Snippets, in rectangles of their own so that neither grows into the
    /// other (R-SQL-84).
    fn side(&mut self, ui: &mut Ui, around: Around<'_>) {
        let full = ui.available_rect_before_wrap();
        let both = self.side.libraries_open && self.side.snippets_open;
        let divider = if both { layout::DIVIDER } else { 0.0 };
        let room = full.height() - 2.0 * HEADER - divider;
        let (libraries, snippets) = section_heights(self.side, room);
        let mut top = full.top();
        let mut next = |height: f32| {
            let rect = Rect::from_min_size(
                egui::pos2(full.left(), top),
                egui::vec2(full.width(), height),
            );
            top += height;
            rect
        };
        let libraries_header = next(HEADER);
        let libraries_body = next(libraries);
        let divider_rect = next(divider);
        let snippets_header = next(HEADER);
        let snippets_body = next(snippets);

        self.in_rect(
            ui,
            libraries_header,
            "sql-side-libraries-header",
            |w, ui| {
                w.section_header(ui, Section::Explorer);
            },
        );
        if self.side.libraries_open {
            self.in_rect(ui, libraries_body, "sql-side-libraries", |w, ui| {
                w.libraries(ui);
            });
        }
        if both {
            match layout::divider(ui, divider_rect, Axis::Sections) {
                Some(Drag::Moved(by)) => {
                    self.side.libraries_share = percent(libraries + by, room.max(0.0));
                }
                Some(Drag::Reset) => self.side.libraries_share = DEFAULT_SHARE,
                None => {}
            }
        }
        self.in_rect(ui, snippets_header, "sql-side-snippets-header", |w, ui| {
            w.section_header(ui, Section::Snippets);
        });
        if self.side.snippets_open {
            self.in_rect(ui, snippets_body, "sql-side-snippets", |w, ui| {
                if let Some(used) = w.snippets.show(ui, around.snippets, around.shared) {
                    w.use_snippet(used);
                }
            });
        }
        ui.allocate_rect(full, egui::Sense::hover());
    }

    /// Draws `add` in `rect` of `ui`, and nowhere else.
    fn in_rect(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        salt: &str,
        add: impl FnOnce(&mut Self, &mut Ui),
    ) {
        let mut child = ui.new_child(UiBuilder::new().id_salt(salt).max_rect(rect));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        add(self, &mut child);
    }

    /// A section's header: its caret and name fold it; Libraries' has New group and the
    /// button that folds the panel, Snippets' New snippet (R-SQL-106).
    fn section_header(&mut self, ui: &mut Ui, section: Section) {
        let open = match section {
            Section::Explorer => self.side.libraries_open,
            Section::Snippets => self.side.snippets_open,
        };
        let mut toggle = false;
        ui.horizontal_centered(|ui| {
            let (caret, verb) = if open {
                (icons::CARET_DOWN, "Fold")
            } else {
                (icons::CARET_RIGHT, "Open")
            };
            toggle |= panel_button(ui, caret, &format!("{verb} {}", section.name()));
            let label = ui
                .scope(|ui| theme::section_label(ui, section.name()))
                .response
                .interact(egui::Sense::click());
            toggle |= label.clicked();
            if section == Section::Explorer {
                theme::button_row(ui, |ui| {
                    if panel_button(ui, icons::CARET_LEFT, "Hide the side panel") {
                        self.toggle_side_panel();
                    }
                    if panel_button(ui, icons::PLUS, "New group…") {
                        self.side.libraries_open = true;
                        self.browser.new_group();
                    }
                });
            } else {
                theme::button_row(ui, |ui| {
                    // R-SQL-106: a new snippet, from the selection or empty.
                    if panel_button(ui, icons::PLUS, "New snippet…") {
                        self.side.snippets_open = true;
                        let selected = self.scripts.selected_text().unwrap_or_default();
                        self.snippets.new_snippet(&selected);
                    }
                });
            }
        });
        if toggle {
            match section {
                Section::Explorer => self.side.libraries_open = !open,
                Section::Snippets => self.side.snippets_open = !open,
            }
        }
    }

    /// The schema browser with the library groups (R-SQL-34, R-SQL-83).
    fn libraries(&mut self, ui: &mut Ui) {
        let (list, server) = self
            .job
            .as_ref()
            .map(|j| (j.libraries.clone(), j.server.clone()))
            .unwrap_or_default();
        // R-SQL-83: the IBM i's name, or its host's until it is read.
        let system = if server.is_empty() {
            self.config.host.clone()
        } else {
            server
        };
        let about = match &self.connected {
            Some(c) => format!("{} · IBM i {}", self.config.host, c.release),
            None => self.config.host.clone(),
        };
        let views: Vec<_> = services::View::ALL
            .into_iter()
            .map(|v| (v, self.system.label(v)))
            .collect();
        let root = browser::Root {
            name: &system,
            about: &about,
            views: &views,
        };
        if let Some(pick) = self.browser.show(ui, &root, &list, &mut self.database) {
            self.use_pick(pick);
        }
        // R-SQL-78: the System node's lists are read the first time it is open.
        if self.browser.system_open() && !self.system.is_read() {
            self.system.read(&mut self.database);
        }
    }

    /// The layout to keep in the settings.
    pub fn side_layout(&self) -> SideLayout {
        SideLayout {
            library_list_open: self.browser.list_open(),
            system_open: self.browser.system_open(),
            ..self.side
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn both(share: u8) -> SideLayout {
        SideLayout {
            libraries_share: share,
            ..SideLayout::default()
        }
    }

    /// R-SQL-84: both open share the room, each keeping its least height; one open gets it
    /// all; a small panel shares what there is.
    #[test]
    fn the_sections_share_the_height_within_their_limits_r_sql_84() {
        let (l, s) = section_heights(both(60), 500.0);
        assert!((l - 300.0).abs() < 0.01 && (s - 200.0).abs() < 0.01);
        let (l, s) = section_heights(both(5), 500.0);
        assert!((l - MIN_SECTION).abs() < 0.01 && (s - 380.0).abs() < 0.01);
        let (l, s) = section_heights(both(100), 500.0);
        assert!((l - 380.0).abs() < 0.01 && (s - MIN_SECTION).abs() < 0.01);
        let (l, s) = section_heights(both(50), 100.0);
        assert!((l - 50.0).abs() < 0.01 && (s - 50.0).abs() < 0.01);
        let only_snippets = SideLayout {
            libraries_open: false,
            ..SideLayout::default()
        };
        assert_eq!(section_heights(only_snippets, 500.0), (0.0, 500.0));
        let neither = SideLayout {
            snippets_open: false,
            ..only_snippets
        };
        assert_eq!(section_heights(neither, 500.0), (0.0, 0.0));
        assert_eq!(section_heights(both(60), -20.0), (0.0, 0.0));
    }

    /// R-SQL-84: the divider's place is kept as a share in percent.
    #[test]
    fn a_dragged_divider_becomes_a_share() {
        assert_eq!(percent(150.0, 300.0), 50);
        assert_eq!(percent(400.0, 300.0), 100);
        assert_eq!(percent(10.0, 0.0), DEFAULT_SHARE);
    }

    /// R-SQL-82: the layout is kept in the settings, and old settings read as the defaults.
    #[test]
    fn the_layout_is_kept_and_read_back() {
        let layout = SideLayout {
            width: 340,
            folded: true,
            snippets_open: false,
            ..SideLayout::default()
        };
        let json = serde_json::to_string(&layout).expect("serializes");
        let back: SideLayout = serde_json::from_str(&json).expect("reads");
        assert_eq!(back, layout);
        let old: SideLayout = serde_json::from_str("{}").expect("reads");
        assert_eq!(old, SideLayout::default());
    }
}
