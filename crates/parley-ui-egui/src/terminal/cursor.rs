//! The cursor (R-UI-1, R-UI-28): its shape, insert mode, and blinking.

use egui::{Align2, CornerRadius, Painter, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};

use super::{Grid, Palette};
use parley_core::Session;

/// The cursor's shape, as Settings ▸ Terminal chooses it (R-UI-28).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Bar,
}

impl CursorShape {
    pub const ALL: [(Self, &'static str); 3] = [
        (Self::Block, "Block"),
        (Self::Underline, "Underline"),
        (Self::Bar, "Bar"),
    ];

    /// The shape shown: in insert mode a bar, or a block when the chosen shape is the bar, so
    /// insert mode always looks different.
    pub const fn shown(self, insert: bool) -> Self {
        match (insert, self) {
            (false, shape) => shape,
            (true, Self::Bar) => Self::Block,
            (true, Self::Block | Self::Underline) => Self::Bar,
        }
    }
}

/// Half a blink, in seconds.
const HALF_BLINK: f64 = 0.5;

/// Whether a blinking cursor shows at `now`: always right after a key (`last_key`), then half
/// a second on and half off.
pub fn blink_visible(now: f64, last_key: f64) -> bool {
    let since = now - last_key;
    since < 2.0 * HALF_BLINK || since.rem_euclid(2.0 * HALF_BLINK) < HALF_BLINK
}

/// The time until a blinking cursor changes, for the next frame.
pub fn until_blink_change(now: f64, last_key: f64) -> f64 {
    let since = (now - last_key).max(0.0);
    HALF_BLINK - since.rem_euclid(HALF_BLINK)
}

/// How the cursor shows in this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    /// The screen does not have the keyboard: an outline.
    Outline,
    /// Blinking, and off just now.
    Hidden,
    Shape(CursorShape),
}

/// Draws the cursor at the session's cursor position as `look` says.
pub(super) fn draw(
    painter: &Painter,
    session: &Session,
    grid: &Grid,
    cells: &[parley_core::screen::Cell],
    look: Look,
    palette: &Palette,
) {
    let screen = session.screen();
    let (row, column) = screen.row_column(screen.cursor);
    let rect = grid.cell(row, column);
    let Some(cell) = cells.get(screen.cursor) else {
        return;
    };
    let color = palette.color(cell.style.color);
    let shape = match look {
        Look::Outline => {
            let stroke = Stroke::new(1.0, color);
            painter.rect_stroke(rect, CornerRadius::ZERO, stroke, egui::StrokeKind::Inside);
            return;
        }
        Look::Hidden => return,
        Look::Shape(shape) => shape,
    };
    match shape {
        CursorShape::Underline => {
            let line = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 3.0), rect.max);
            painter.rect_filled(line, CornerRadius::ZERO, color);
        }
        CursorShape::Bar => {
            let bar = Rect::from_min_max(rect.min, Pos2::new(rect.left() + 2.0, rect.bottom()));
            painter.rect_filled(bar, CornerRadius::ZERO, color);
        }
        CursorShape::Block => {
            painter.rect_filled(rect, CornerRadius::ZERO, color);
            if cell.ch != ' ' {
                painter.text(
                    rect.left_top(),
                    Align2::LEFT_TOP,
                    cell.ch,
                    grid.font.clone(),
                    palette.background,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_mode_always_looks_different_r_ui_28() {
        assert_eq!(CursorShape::Block.shown(false), CursorShape::Block);
        assert_eq!(CursorShape::Block.shown(true), CursorShape::Bar);
        assert_eq!(CursorShape::Underline.shown(true), CursorShape::Bar);
        assert_eq!(CursorShape::Bar.shown(false), CursorShape::Bar);
        assert_eq!(CursorShape::Bar.shown(true), CursorShape::Block);
    }

    #[test]
    fn a_blinking_cursor_stays_on_while_typing_then_blinks_r_ui_28() {
        assert!(blink_visible(10.3, 10.0), "right after a key");
        assert!(blink_visible(10.9, 10.0), "a second after a key");
        assert!(blink_visible(11.2, 10.0));
        assert!(!blink_visible(11.7, 10.0));
        assert!(blink_visible(12.1, 10.0));
        assert!((until_blink_change(11.2, 10.0) - 0.3).abs() < 1e-9);
    }
}
