//! How a tab's panes are laid out (R-SSH-40, D47): a tree of splits, side by side or one over the other, each
//! with the share the first part takes. Pure: no drawing, so it is tested on its own.

use egui::{Pos2, Rect, Vec2};

/// A pane's number, unique in the window.
pub type PaneId = u64;

/// The gap between two panes, where the divider is dragged.
pub const DIVIDER: f32 = 6.0;
/// The smallest share a pane can be dragged to.
const MIN_SHARE: f32 = 0.1;

/// Side by side (`Row`) or one over the other (`Column`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Row,
    Column,
}

/// Where to look for the next pane (Alt+arrow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// A pane, or a split of two parts.
#[derive(Debug, Clone, PartialEq)]
pub enum Layout {
    Pane(PaneId),
    Split {
        axis: Axis,
        share: f32,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

impl Layout {
    /// Splits the pane `target` along `axis`, `new` taking the second half. False when `target` is not here.
    pub fn split(&mut self, target: PaneId, new: PaneId, axis: Axis) -> bool {
        match self {
            Self::Pane(id) if *id == target => {
                *self = Self::Split {
                    axis,
                    share: 0.5,
                    first: Box::new(Self::Pane(target)),
                    second: Box::new(Self::Pane(new)),
                };
                true
            }
            Self::Pane(_) => false,
            Self::Split { first, second, .. } => {
                first.split(target, new, axis) || second.split(target, new, axis)
            }
        }
    }

    /// Removes the pane `target`; its split's other part takes its place. False when it is not here or it is the
    /// only pane (a tab always has one).
    pub fn remove(&mut self, target: PaneId) -> bool {
        let Self::Split { first, second, .. } = self else {
            return false;
        };
        let other = if **first == Self::Pane(target) {
            second
        } else if **second == Self::Pane(target) {
            first
        } else {
            return first.remove(target) || second.remove(target);
        };
        *self = (**other).clone();
        true
    }

    /// The panes, first to last.
    pub fn panes(&self) -> Vec<PaneId> {
        match self {
            Self::Pane(id) => vec![*id],
            Self::Split { first, second, .. } => {
                let mut ids = first.panes();
                ids.extend(second.panes());
                ids
            }
        }
    }

    /// Each pane's rectangle in `area`, the dividers' gaps left out.
    pub fn rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::new();
        self.collect_rects(area, &mut out);
        out
    }

    fn collect_rects(&self, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match self {
            Self::Pane(id) => out.push((*id, area)),
            Self::Split {
                axis,
                share,
                first,
                second,
            } => {
                let (a, _, b) = parts(area, *axis, *share);
                first.collect_rects(a, out);
                second.collect_rects(b, out);
            }
        }
    }

    /// Each divider: its rectangle, its axis, the area it divides, and its share to change while dragged.
    pub fn dividers(&mut self, area: Rect) -> Vec<(Rect, Axis, Rect, &mut f32)> {
        let mut out = Vec::new();
        self.collect_dividers(area, &mut out);
        out
    }

    fn collect_dividers<'a>(
        &'a mut self,
        area: Rect,
        out: &mut Vec<(Rect, Axis, Rect, &'a mut f32)>,
    ) {
        if let Self::Split {
            axis,
            share,
            first,
            second,
        } = self
        {
            let (a, divider, b) = parts(area, *axis, *share);
            out.push((divider, *axis, area, share));
            first.collect_dividers(a, out);
            second.collect_dividers(b, out);
        }
    }

    /// The pane next to `from` in `direction`: of the panes beyond its edge that overlap it across, the nearest,
    /// then the one most in line.
    pub fn neighbour(&self, area: Rect, from: PaneId, direction: Direction) -> Option<PaneId> {
        let rects = self.rects(area);
        let (_, here) = rects.iter().find(|(id, _)| *id == from)?;
        rects
            .iter()
            .filter(|(id, _)| *id != from)
            .filter_map(|(id, r)| {
                let (gap, overlap) = match direction {
                    Direction::Left => (
                        here.left() - r.right(),
                        overlap(here.y_range(), r.y_range()),
                    ),
                    Direction::Right => (
                        r.left() - here.right(),
                        overlap(here.y_range(), r.y_range()),
                    ),
                    Direction::Up => (
                        here.top() - r.bottom(),
                        overlap(here.x_range(), r.x_range()),
                    ),
                    Direction::Down => (
                        r.top() - here.bottom(),
                        overlap(here.x_range(), r.x_range()),
                    ),
                };
                (gap >= -0.5 && overlap > 0.0).then_some((*id, gap, overlap))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(b.2.total_cmp(&a.2)))
            .map(|(id, _, _)| id)
    }
}

/// How much two ranges overlap.
fn overlap(a: egui::Rangef, b: egui::Rangef) -> f32 {
    (a.max.min(b.max) - a.min.max(b.min)).max(0.0)
}

/// The first part, the divider and the second part of `area` split at `share`.
fn parts(area: Rect, axis: Axis, share: f32) -> (Rect, Rect, Rect) {
    let share = share.clamp(MIN_SHARE, 1.0 - MIN_SHARE);
    match axis {
        Axis::Row => {
            let first = ((area.width() - DIVIDER) * share).max(0.0);
            let a = Rect::from_min_size(area.min, Vec2::new(first, area.height()));
            let divider = Rect::from_min_size(
                Pos2::new(a.right(), area.top()),
                Vec2::new(DIVIDER, area.height()),
            );
            let b = Rect::from_min_max(Pos2::new(divider.right(), area.top()), area.max);
            (a, divider, b)
        }
        Axis::Column => {
            let first = ((area.height() - DIVIDER) * share).max(0.0);
            let a = Rect::from_min_size(area.min, Vec2::new(area.width(), first));
            let divider = Rect::from_min_size(
                Pos2::new(area.left(), a.bottom()),
                Vec2::new(area.width(), DIVIDER),
            );
            let b = Rect::from_min_max(Pos2::new(area.left(), divider.bottom()), area.max);
            (a, divider, b)
        }
    }
}

/// The share the first part gets with the divider dragged to `pointer` in `area`.
pub fn share_at(area: Rect, axis: Axis, pointer: Pos2) -> f32 {
    let share = match axis {
        Axis::Row => (pointer.x - area.left()) / area.width().max(1.0),
        Axis::Column => (pointer.y - area.top()) / area.height().max(1.0),
    };
    share.clamp(MIN_SHARE, 1.0 - MIN_SHARE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(806.0, 406.0))
    }

    /// 1 | 2 over 3.
    fn three() -> Layout {
        let mut layout = Layout::Pane(1);
        assert!(layout.split(1, 2, Axis::Row));
        assert!(layout.split(2, 3, Axis::Column));
        layout
    }

    #[test]
    fn splitting_puts_the_new_pane_beside_or_below_r_ssh_40() {
        let layout = three();
        assert_eq!(layout.panes(), [1, 2, 3]);
        let rects = layout.rects(area());
        assert_eq!(
            rects[0].1,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 406.0))
        );
        assert_eq!(rects[1].1.left(), 406.0, "after the divider");
        assert_eq!(rects[2].1.top(), rects[1].1.bottom() + DIVIDER);
        assert!(!Layout::Pane(1).split(9, 2, Axis::Row), "no such pane");
    }

    #[test]
    fn closing_a_pane_gives_its_room_to_the_other_part_r_ssh_40() {
        let mut layout = three();
        assert!(layout.remove(2));
        assert_eq!(layout.panes(), [1, 3]);
        assert!(layout.remove(1));
        assert_eq!(layout, Layout::Pane(3));
        assert!(!layout.remove(3), "a tab keeps its last pane");
        assert!(!three().remove(9));
    }

    #[test]
    fn alt_arrows_find_the_pane_in_that_direction_r_ssh_40() {
        let layout = three();
        let n = |from, d| layout.neighbour(area(), from, d);
        assert_eq!(
            n(1, Direction::Right),
            Some(2),
            "the one most in line: the top one"
        );
        assert_eq!(n(2, Direction::Left), Some(1));
        assert_eq!(n(3, Direction::Left), Some(1));
        assert_eq!(n(2, Direction::Down), Some(3));
        assert_eq!(n(3, Direction::Up), Some(2));
        assert_eq!(n(1, Direction::Left), None);
        assert_eq!(n(1, Direction::Up), None);
        assert_eq!(n(9, Direction::Up), None);
    }

    #[test]
    fn dragging_a_divider_changes_its_share_within_limits_r_ssh_40() {
        let mut layout = three();
        let dividers = layout.dividers(area());
        assert_eq!(dividers.len(), 2);
        let (_, axis, divided, share) = dividers.into_iter().next().expect("the first split");
        *share = share_at(divided, axis, Pos2::new(200.0, 10.0));
        // The share of 200 of 806 points, of the width less the divider.
        assert!((layout.rects(area())[0].1.width() - 800.0 * 200.0 / 806.0).abs() < 0.5);
        assert!((share_at(area(), Axis::Row, Pos2::new(-50.0, 0.0)) - 0.1).abs() < f32::EPSILON);
        assert!(
            (share_at(area(), Axis::Column, Pos2::new(0.0, 9999.0)) - 0.9).abs() < f32::EPSILON
        );
    }
}
