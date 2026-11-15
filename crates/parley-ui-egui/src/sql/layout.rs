//! Where the SQL window's results go: below the editor, beside it, or beside it when there is
//! room (R-SQL-49), as Tortuga places a response (D12). Beside, a divider between the two can
//! be dragged; double-clicking it shares the width equally again.

use egui::{Rect, Sense, Ui};
use serde::{Deserialize, Serialize};

use crate::theme;

/// Where the results go relative to the editor, as the settings keep it (R-SQL-49).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResultsPlace {
    /// The editor above, the results below.
    #[default]
    Below,
    /// The editor on the left, the results on the right.
    Beside,
    /// Beside when the window is wide enough, else below.
    Automatic,
}

/// How editor and results are arranged this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Arrangement {
    Stacked,
    SideBySide,
}

/// "Automatic" puts the results beside the editor from this width of the work area on.
pub(super) const BESIDE_FROM: f32 = 1100.0;
/// Beside: the editor's share until the divider is first dragged, and the least width either
/// side gets.
pub(super) const DEFAULT_SHARE: f32 = 0.5;
const MIN_COLUMN_WIDTH: f32 = 320.0;
/// The divider's thickness; the line is drawn in its middle.
pub(super) const DIVIDER: f32 = 13.0;

impl ResultsPlace {
    /// The arrangement for a work area `width` wide.
    pub(super) fn arrangement(self, width: f32) -> Arrangement {
        match self {
            Self::Beside => Arrangement::SideBySide,
            Self::Automatic if width >= BESIDE_FROM => Arrangement::SideBySide,
            Self::Below | Self::Automatic => Arrangement::Stacked,
        }
    }
}

/// Beside: the editor's width of `available`, for the editor's `share` of it.
pub(super) fn editor_width(share: f32, available: f32) -> f32 {
    (available * share)
        .min(available - MIN_COLUMN_WIDTH - DIVIDER)
        .max(MIN_COLUMN_WIDTH)
}

/// Below: the least height of the editor's text, and of the results under it (R-SQL-50).
const MIN_TEXT_HEIGHT: f32 = 90.0;
const MIN_RESULTS_HEIGHT: f32 = 120.0;

/// How high the editor's text is drawn: at least `least`, and scrolled past `most`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TextHeight {
    pub least: f32,
    pub most: f32,
}

/// Below: the height of the editor's text in a work area `available` tall (R-SQL-50). Until
/// the divider is dragged (`chosen` is `None`) the text is as high as it needs, up to a share
/// of the area; once dragged, the `chosen` height, leaving the results their least height.
/// `below_text` is what the editor needs under its text: the tab strip, the Run buttons and a
/// line of error.
pub(super) fn editor_height(chosen: Option<f32>, available: f32, below_text: f32) -> TextHeight {
    let room = (available - below_text - DIVIDER - MIN_RESULTS_HEIGHT).max(MIN_TEXT_HEIGHT);
    match chosen {
        None => TextHeight {
            least: 0.0,
            most: (available * 0.38).clamp(MIN_TEXT_HEIGHT, 320.0).min(room),
        },
        Some(chosen) => {
            let height = chosen.min(room).max(MIN_TEXT_HEIGHT);
            TextHeight {
                least: height,
                most: height,
            }
        }
    }
}

/// Which way a divider lies between editor and results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
    /// A vertical line between the editor on the left and the results on the right.
    Columns,
    /// A horizontal line between the editor above and the results below.
    Rows,
    /// A horizontal line between Libraries and Snippets in the side panel (R-SQL-84).
    Sections,
}

/// What was done to a divider this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Drag {
    /// Dragged by this many points, right or down.
    Moved(f32),
    /// Double-clicked: back to the usual size.
    Reset,
}

/// The divider in `rect` between editor and results (R-SQL-49, R-SQL-50).
pub(super) fn divider(ui: &Ui, rect: Rect, axis: Axis) -> Option<Drag> {
    let (salt, hint, cursor) = match axis {
        Axis::Columns => (
            "sql-results-divider",
            "Drag to change the widths; double-click to share them equally.",
            egui::CursorIcon::ResizeHorizontal,
        ),
        Axis::Rows => (
            "sql-editor-height-divider",
            "Drag to change the editor's height; double-click for the usual height.",
            egui::CursorIcon::ResizeVertical,
        ),
        Axis::Sections => (
            "sql-side-sections-divider",
            "Drag to change the heights; double-click for the usual heights.",
            egui::CursorIcon::ResizeVertical,
        ),
    };
    let response = ui
        .interact(rect, ui.make_persistent_id(salt), Sense::click_and_drag())
        .on_hover_text(hint);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::accesskit::Role::Splitter,
            true,
            match axis {
                Axis::Columns | Axis::Rows => "Editor and results divider",
                Axis::Sections => "Libraries and Snippets divider",
            },
        )
    });
    let p = theme::palette(ui);
    let active = response.hovered() || response.dragged();
    let stroke = if active {
        egui::Stroke::new(2.0, p.accent)
    } else {
        egui::Stroke::new(1.0, p.border)
    };
    match axis {
        Axis::Columns => {
            ui.painter().vline(rect.center().x, rect.y_range(), stroke);
        }
        Axis::Rows | Axis::Sections => {
            ui.painter().hline(rect.x_range(), rect.center().y, stroke);
        }
    }
    if active {
        ui.ctx().set_cursor_icon(cursor);
    }
    if response.double_clicked() {
        Some(Drag::Reset)
    } else if response.dragged() {
        let delta = response.drag_delta();
        Some(Drag::Moved(match axis {
            Axis::Columns => delta.x,
            Axis::Rows | Axis::Sections => delta.y,
        }))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-SQL-49: automatic is below in a narrow window and beside in a wide one; the other two
    /// don't look at the width.
    #[test]
    fn automatic_puts_the_results_beside_only_when_there_is_room() {
        assert_eq!(
            ResultsPlace::Automatic.arrangement(800.0),
            Arrangement::Stacked
        );
        assert_eq!(
            ResultsPlace::Automatic.arrangement(1400.0),
            Arrangement::SideBySide
        );
        assert_eq!(
            ResultsPlace::Below.arrangement(2000.0),
            Arrangement::Stacked
        );
        assert_eq!(
            ResultsPlace::Beside.arrangement(500.0),
            Arrangement::SideBySide
        );
    }

    /// R-SQL-49: either side keeps its least width; a window too narrow for both keeps the
    /// editor's.
    #[test]
    fn the_editor_width_stays_within_its_limits() {
        assert!((editor_width(0.5, 1000.0) - 500.0).abs() < f32::EPSILON);
        assert!((editor_width(0.05, 1000.0) - MIN_COLUMN_WIDTH).abs() < f32::EPSILON);
        let most = 1000.0 - MIN_COLUMN_WIDTH - DIVIDER;
        assert!((editor_width(0.99, 1000.0) - most).abs() < f32::EPSILON);
        assert!((editor_width(0.5, 400.0) - MIN_COLUMN_WIDTH).abs() < f32::EPSILON);
    }

    /// R-SQL-50: below, the editor's text is as high as it needs until the divider is dragged,
    /// then the dragged height, leaving the results their least height; a small window keeps
    /// the editor's least height.
    #[test]
    fn the_editor_height_follows_the_divider_within_its_limits() {
        let exactly = |h: f32| TextHeight { least: h, most: h };
        let usual = editor_height(None, 600.0, 110.0);
        assert!(usual.least.abs() < 0.01 && (usual.most - 228.0).abs() < 0.01);
        assert_eq!(editor_height(Some(300.0), 600.0, 110.0), exactly(300.0));
        assert_eq!(
            editor_height(Some(10.0), 600.0, 110.0),
            exactly(MIN_TEXT_HEIGHT)
        );
        let room = 600.0 - 110.0 - DIVIDER - MIN_RESULTS_HEIGHT;
        assert_eq!(editor_height(Some(900.0), 600.0, 110.0), exactly(room));
        assert_eq!(
            editor_height(Some(200.0), 200.0, 110.0),
            exactly(MIN_TEXT_HEIGHT)
        );
    }

    /// The setting is kept by name, and old settings without it read as Below.
    #[test]
    fn the_place_is_kept_by_name() {
        let json = serde_json::to_string(&ResultsPlace::Automatic).expect("serializes");
        assert_eq!(json, "\"automatic\"");
        let back: ResultsPlace = serde_json::from_str("\"beside\"").expect("reads");
        assert_eq!(back, ResultsPlace::Beside);
    }
}
