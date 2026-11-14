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

/// The divider in `rect`, after an editor `left` wide of `available` in all. Returns the
/// editor's new share when it was dragged or double-clicked.
pub(super) fn divider(ui: &Ui, rect: Rect, left: f32, available: f32) -> Option<f32> {
    let id = ui.make_persistent_id("sql-results-divider");
    let response = ui
        .interact(rect, id, Sense::click_and_drag())
        .on_hover_text("Drag to change the widths; double-click to share them equally.");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::accesskit::Role::Splitter,
            true,
            "Editor and results divider",
        )
    });
    let p = theme::palette(ui);
    let active = response.hovered() || response.dragged();
    let stroke = if active {
        egui::Stroke::new(2.0, p.accent)
    } else {
        egui::Stroke::new(1.0, p.border)
    };
    ui.painter().vline(rect.center().x, rect.y_range(), stroke);
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.double_clicked() {
        Some(DEFAULT_SHARE)
    } else if response.dragged() && available > 0.0 {
        Some(((left + response.drag_delta().x) / available).clamp(0.0, 1.0))
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

    /// The setting is kept by name, and old settings without it read as Below.
    #[test]
    fn the_place_is_kept_by_name() {
        let json = serde_json::to_string(&ResultsPlace::Automatic).expect("serializes");
        assert_eq!(json, "\"automatic\"");
        let back: ResultsPlace = serde_json::from_str("\"beside\"").expect("reads");
        assert_eq!(back, ResultsPlace::Beside);
    }
}
