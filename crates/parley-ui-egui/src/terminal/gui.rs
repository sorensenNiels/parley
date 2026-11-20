//! GUI selection fields and scroll bars on the screen (R-DS-11, R-DS-12, D57). The engine
//! writes a selection field's choices as text; this draws a radio button or check box over each
//! choice's indicator, a box round each push button, and an underline under each mnemonic. A
//! scroll bar is drawn over its positions with arrows at its ends; a click on an arrow moves it
//! one row, a click beside the slider a page, and dragging the slider anywhere.

use egui::{CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui, Vec2};
use parley_core::Session;
use parley_core::screen::{ChoiceState, Indicator, Screen, ScrollBar, SelectionField, Style};
use parley_core::session::{MouseAction, MouseButton, MouseEvent};

use super::{Grid, Palette};

/// Draws every selection field's widgets and every scroll bar.
pub(super) fn draw(painter: &Painter, grid: &Grid, screen: &Screen, palette: &Palette) {
    for field in screen.selection_fields() {
        draw_field(painter, grid, screen, field, palette);
    }
    for bar in screen.scroll_bars() {
        draw_bar(painter, grid, bar, palette);
    }
}

fn draw_field(
    painter: &Painter,
    grid: &Grid,
    screen: &Screen,
    field: &SelectionField,
    palette: &Palette,
) {
    let columns = grid.columns;
    let rect_of = |address: usize| grid.cell(address / columns, address % columns);
    for (i, choice) in field.choices.iter().enumerate() {
        let attribute = match choice.state {
            ChoiceState::Available => field.colors.available,
            ChoiceState::Selected => field.colors.selected,
            ChoiceState::Unavailable => field.colors.unavailable,
        };
        let mut color = palette.color(Style::from_attribute(attribute).color);
        if choice.state == ChoiceState::Unavailable {
            color = color.gamma_multiply(0.5);
        }
        let stroke = Stroke::new(1.5, color);
        let selected = choice.state == ChoiceState::Selected;
        let Some(text) = field.text_start(i) else {
            continue;
        };
        match field.indicator {
            Indicator::Mark => {
                let cell = rect_of(text - 2);
                painter.rect_filled(cell, CornerRadius::ZERO, palette.background);
                mark(painter, cell, field.kind.multiple(), selected, stroke);
            }
            Indicator::Button => {
                let first = rect_of(choice.address + 1);
                let last = rect_of(choice.address + field.choice_width() - 2);
                let button = Rect::from_min_max(first.min, last.max).shrink(1.0);
                if selected || field.choice_at(screen.cursor) == Some(i) {
                    painter.rect_filled(button, CornerRadius::same(3), color.gamma_multiply(0.15));
                }
                painter.rect_stroke(button, CornerRadius::same(3), stroke, StrokeKind::Inside);
            }
            Indicator::None => {}
        }
        if let Some(m) = choice.mnemonic.filter(|&m| m < field.text_size) {
            let cell = rect_of(text + m);
            painter.hline(cell.x_range(), cell.bottom() - 1.0, Stroke::new(1.0, color));
        }
    }
}

/// A radio button (a circle, with a dot when selected) or a check box (a square, with a tick).
fn mark(painter: &Painter, cell: Rect, check_box: bool, selected: bool, stroke: Stroke) {
    let size = cell.width().min(cell.height()) * 0.8;
    let r = Rect::from_center_size(cell.center(), Vec2::splat(size));
    if check_box {
        painter.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
        if selected {
            let p = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);
            painter.add(Shape::line(
                vec![p(0.22, 0.52), p(0.42, 0.72), p(0.78, 0.3)],
                stroke,
            ));
        }
    } else {
        painter.circle_stroke(r.center(), size / 2.0 - 1.0, stroke);
        if selected {
            painter.circle_filled(r.center(), size / 4.0, stroke.color);
        }
    }
}

/// The screen rectangle a scroll bar takes.
fn bar_rect(grid: &Grid, bar: &ScrollBar) -> Rect {
    let area = bar.area(grid.columns);
    Rect::from_min_max(
        grid.cell(area.top, area.left).min,
        grid.cell(area.bottom, area.right).max,
    )
}

/// The length of a scroll bar's arrow at each end: one position.
fn arrow_length(grid: &Grid, bar: &ScrollBar) -> f32 {
    if bar.horizontal {
        grid.cell_size.x
    } else {
        grid.cell_size.y
    }
}

/// The track between the arrows and the slider in it, as fractions of the track from its start.
#[expect(
    clippy::cast_precision_loss,
    reason = "row counts far below 2^24 in practice"
)]
fn slider(bar: &ScrollBar) -> (f32, f32) {
    let total = bar.total.max(1) as f32;
    let start = bar.position as f32 / total;
    let length = (bar.size as f32 / total).clamp(0.05, 1.0);
    (start.min(1.0 - length), (start + length).min(1.0))
}

fn track(grid: &Grid, bar: &ScrollBar) -> Rect {
    let rect = bar_rect(grid, bar);
    let arrow = arrow_length(grid, bar);
    if bar.horizontal {
        Rect::from_min_max(
            rect.min + Vec2::new(arrow, 0.0),
            rect.max - Vec2::new(arrow, 0.0),
        )
    } else {
        Rect::from_min_max(
            rect.min + Vec2::new(0.0, arrow),
            rect.max - Vec2::new(0.0, arrow),
        )
    }
}

fn draw_bar(painter: &Painter, grid: &Grid, bar: &ScrollBar, palette: &Palette) {
    let rect = bar_rect(grid, bar);
    let color = palette.color(parley_core::Color::Blue);
    painter.rect_filled(rect, CornerRadius::ZERO, palette.background);
    let track = track(grid, bar);
    painter.rect_filled(track, CornerRadius::same(2), color.gamma_multiply(0.2));
    let (from, to) = slider(bar);
    let slider = if bar.horizontal {
        Rect::from_x_y_ranges(
            track.left() + track.width() * from..=track.left() + track.width() * to,
            track.y_range(),
        )
    } else {
        Rect::from_x_y_ranges(
            track.x_range(),
            track.top() + track.height() * from..=track.top() + track.height() * to,
        )
    };
    painter.rect_filled(slider.shrink(1.0), CornerRadius::same(2), color);
    let arrow = arrow_length(grid, bar);
    let (back, forward) = if bar.horizontal {
        (
            Rect::from_min_size(rect.min, Vec2::new(arrow, rect.height())),
            Rect::from_min_max(Pos2::new(rect.right() - arrow, rect.top()), rect.max),
        )
    } else {
        (
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), arrow)),
            Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - arrow), rect.max),
        )
    };
    triangle(painter, back, bar.horizontal, true, color);
    triangle(painter, forward, bar.horizontal, false, color);
}

/// An arrow pointing back (up or left) or on (down or right).
fn triangle(painter: &Painter, cell: Rect, horizontal: bool, back: bool, color: egui::Color32) {
    let c = cell.center();
    let h = cell.width().min(cell.height()) * 0.3;
    let points = match (horizontal, back) {
        (false, true) => vec![
            c + Vec2::new(0.0, -h),
            c + Vec2::new(h, h),
            c + Vec2::new(-h, h),
        ],
        (false, false) => vec![
            c + Vec2::new(0.0, h),
            c + Vec2::new(-h, -h),
            c + Vec2::new(h, -h),
        ],
        (true, true) => vec![
            c + Vec2::new(-h, 0.0),
            c + Vec2::new(h, h),
            c + Vec2::new(h, -h),
        ],
        (true, false) => vec![
            c + Vec2::new(h, 0.0),
            c + Vec2::new(-h, -h),
            c + Vec2::new(-h, h),
        ],
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Where on a scroll bar the pointer is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BarPart {
    Back,
    Forward,
    /// On the track, this far along it (0 to 1).
    Track(f32),
}

/// The scroll bar under `pos`, and where on it.
pub(super) fn bar_at(screen: &Screen, grid: &Grid, pos: Pos2) -> Option<(usize, BarPart)> {
    screen
        .scroll_bars()
        .iter()
        .enumerate()
        .find_map(|(i, bar)| {
            if !bar_rect(grid, bar).contains(pos) {
                return None;
            }
            let track = track(grid, bar);
            let (at, start, end) = if bar.horizontal {
                (pos.x, track.left(), track.right())
            } else {
                (pos.y, track.top(), track.bottom())
            };
            let part = if at < start {
                BarPart::Back
            } else if at > end {
                BarPart::Forward
            } else {
                BarPart::Track(along(bar, grid, pos))
            };
            Some((i, part))
        })
}

/// How far along `bar`'s track `pos` is, from 0 to 1, also when it is beside the bar (a drag
/// that went past it).
pub(super) fn along(bar: &ScrollBar, grid: &Grid, pos: Pos2) -> f32 {
    let track = track(grid, bar);
    let (at, start, length) = if bar.horizontal {
        (pos.x, track.left(), track.width())
    } else {
        (pos.y, track.top(), track.height())
    };
    ((at - start) / length.max(1.0)).clamp(0.0, 1.0)
}

/// Where a click on `part` moves the slider: one row for an arrow, a page beside the slider.
pub(super) fn clicked(bar: &ScrollBar, part: BarPart) -> u32 {
    let page = u32::try_from(bar.size).unwrap_or(1).max(1);
    match part {
        BarPart::Back => bar.position.saturating_sub(1),
        BarPart::Forward => bar.position.saturating_add(1),
        BarPart::Track(at) => {
            let (from, to) = slider(bar);
            if at < from {
                bar.position.saturating_sub(page)
            } else if at > to {
                bar.position.saturating_add(page)
            } else {
                bar.position
            }
        }
    }
}

/// Where dragging the slider to `part` moves it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "a fraction from 0 to 1 of a row count"
)]
pub(super) fn dragged(bar: &ScrollBar, part: BarPart) -> u32 {
    match part {
        BarPart::Back => 0,
        BarPart::Forward => bar.last_position(),
        BarPart::Track(at) => (at * bar.last_position() as f32).round() as u32,
    }
}

/// Gives the session this frame's mouse events over the screen, for the host's programmable
/// mouse buttons (R-DS-12). True if one of them took an event: the click then does nothing else.
pub(super) fn mouse(
    ui: &Ui,
    response: &egui::Response,
    grid: &Grid,
    session: &mut Session,
) -> bool {
    let double = response.double_clicked();
    let events = ui.input(|i| i.events.clone());
    let mut taken = false;
    let mut send = |event: MouseEvent, pos: Pos2| {
        if let Some((row, column)) = grid.cell_at(pos) {
            taken |= session.mouse(event, row, column);
        }
    };
    for event in events {
        let egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        } = event
        else {
            continue;
        };
        let button = match button {
            egui::PointerButton::Primary => MouseButton::Left,
            egui::PointerButton::Secondary => MouseButton::Right,
            egui::PointerButton::Middle => MouseButton::Middle,
            egui::PointerButton::Extra1 | egui::PointerButton::Extra2 => continue,
        };
        let action = if pressed {
            MouseAction::Press
        } else {
            MouseAction::Release
        };
        if response.rect.contains(pos) {
            send(
                MouseEvent {
                    button,
                    action,
                    shift: modifiers.shift,
                },
                pos,
            );
        }
    }
    if double && let Some(pos) = response.interact_pointer_pos() {
        let shift = ui.input(|i| i.modifiers.shift);
        send(
            MouseEvent {
                button: MouseButton::Left,
                action: MouseAction::DoubleClick,
                shift,
            },
            pos,
        );
    }
    taken
}

/// True over a choice of a selection field: the hand pointer shows.
pub(super) fn on_choice(screen: &Screen, row: usize, column: usize, columns: usize) -> bool {
    let address = row * columns + column;
    screen
        .selection_fields()
        .iter()
        .any(|f| f.choice_at(address).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(position: u32) -> ScrollBar {
        ScrollBar {
            start: 79,
            horizontal: false,
            total: 100,
            position,
            size: 10,
            move_cursor: false,
            modified: false,
        }
    }

    #[test]
    fn arrows_move_a_row_and_the_track_a_page_r_ds_12() {
        let b = bar(20);
        assert_eq!(clicked(&b, BarPart::Back), 19);
        assert_eq!(clicked(&b, BarPart::Forward), 21);
        assert_eq!(clicked(&b, BarPart::Track(0.0)), 10, "above the slider");
        assert_eq!(clicked(&b, BarPart::Track(0.9)), 30, "below it");
        assert_eq!(clicked(&b, BarPart::Track(0.25)), 20, "on it");
        assert_eq!(clicked(&bar(0), BarPart::Back), 0);
    }

    #[test]
    fn dragging_the_slider_goes_to_that_share_of_the_rows_r_ds_12() {
        let b = bar(0);
        assert_eq!(dragged(&b, BarPart::Track(0.5)), 45);
        assert_eq!(dragged(&b, BarPart::Track(1.0)), 90, "the last screenful");
        assert_eq!(dragged(&b, BarPart::Back), 0);
    }
}
