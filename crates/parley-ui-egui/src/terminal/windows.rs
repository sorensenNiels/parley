//! GUI windows drawn as frames (R-DS-10): where the screen holds a window's border characters
//! (`.` and `:`), Parley draws a line in the border's colour instead; the title and footer stay
//! readable in the line's gaps.

use egui::{CornerRadius, Painter, Pos2, Stroke};
use parley_core::screen::{Screen, Style, Window};

use super::{Grid, Palette};

/// Which part of the frame a position draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    Horizontal,
    Vertical,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// The positions of `window`'s frame that hold border characters, with the part each draws.
pub(super) fn frame(screen: &Screen, window: &Window) -> Vec<(usize, usize, Part)> {
    let outer = window.outer();
    let (top, bottom, left, right) = (outer.top, outer.bottom, outer.left + 1, outer.right - 1);
    let columns = usize::from(screen.size().columns());
    let is_border = |r: usize, c: usize| window.spec.chars.contains(&screen.get(r * columns + c));
    let mut parts = Vec::new();
    for c in left..=right {
        for (r, first, last) in [
            (top, Part::TopLeft, Part::TopRight),
            (bottom, Part::BottomLeft, Part::BottomRight),
        ] {
            let part = match c {
                _ if c == left => first,
                _ if c == right => last,
                _ => Part::Horizontal,
            };
            if is_border(r, c) {
                parts.push((r, c, part));
            }
        }
    }
    for r in top + 1..bottom {
        for c in [left, right] {
            if is_border(r, c) {
                parts.push((r, c, Part::Vertical));
            }
        }
    }
    parts
}

/// Draws every window's frame over the screen's border characters.
pub(super) fn draw(painter: &Painter, grid: &Grid, screen: &Screen, palette: &Palette) {
    for window in screen.windows() {
        let color = palette.color(Style::from_attribute(window.spec.attribute).color);
        let stroke = Stroke::new(1.5, color);
        for (r, c, part) in frame(screen, window) {
            let cell = grid.cell(r, c);
            painter.rect_filled(cell, CornerRadius::ZERO, palette.background);
            let mid = cell.center();
            let (west, east) = (
                Pos2::new(cell.left(), mid.y),
                Pos2::new(cell.right(), mid.y),
            );
            let (north, south) = (
                Pos2::new(mid.x, cell.top()),
                Pos2::new(mid.x, cell.bottom()),
            );
            let lines: &[(Pos2, Pos2)] = match part {
                Part::Horizontal => &[(west, east)],
                Part::Vertical => &[(north, south)],
                Part::TopLeft => &[(mid, east), (mid, south)],
                Part::TopRight => &[(west, mid), (mid, south)],
                Part::BottomLeft => &[(mid, east), (north, mid)],
                Part::BottomRight => &[(west, mid), (north, mid)],
            };
            for &(a, b) in lines {
                painter.line_segment([a, b], stroke);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parley_core::CodePage;
    use parley_core::screen::{Align, Caption, DEFAULT_BORDER_ATTRIBUTE, ScreenSize, WindowSpec};

    #[test]
    fn the_frame_covers_border_characters_but_not_the_title_r_ds_10() {
        let page = CodePage::Ccsid37;
        let b = |c| page.to_byte(c).unwrap_or(0);
        let mut screen = Screen::new(ScreenSize::Standard);
        let spec = WindowSpec {
            row: 2,
            column: 4,
            depth: 2,
            width: 6,
            restrict_cursor: false,
            pull_down: false,
            attribute: DEFAULT_BORDER_ATTRIBUTE,
            chars: [
                b('.'),
                b('.'),
                b('.'),
                b(':'),
                b(':'),
                b(':'),
                b('.'),
                b(':'),
            ],
            title: Some(Caption {
                text: "Ab".chars().map(b).collect(),
                align: Align::Center,
                attribute: None,
            }),
            footer: None,
        };
        screen.create_window(spec);
        let window = &screen.windows()[0];
        let parts = frame(&screen, window);
        // Top row columns 5..=14: 10 positions, 2 of them the title.
        let top = parts.iter().filter(|(r, _, _)| *r == 2).count();
        assert_eq!(top, 8);
        assert!(parts.contains(&(2, 5, Part::TopLeft)));
        assert!(parts.contains(&(5, 14, Part::BottomRight)));
        assert!(
            parts.contains(&(3, 5, Part::Vertical)) && parts.contains(&(4, 14, Part::Vertical))
        );
        assert!(
            !parts
                .iter()
                .any(|&(r, c, _)| r == 2 && (9..=10).contains(&c)),
            "title"
        );
    }
}
