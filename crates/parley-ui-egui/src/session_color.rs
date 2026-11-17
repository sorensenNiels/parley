//! A colour for each session (R-UI-35, R-SSH-61, D54), shared by Parley 5250 and Parley SSH: chosen in the session
//! dialog, shown as stripes, lines, an outline and a frame, never as a background. The colours themselves are theme
//! tokens ([`theme::session_color`]).

use egui::{Color32, Frame, Panel, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use serde::{Deserialize, Serialize};

use crate::theme;

/// A session's colour: none, or one of eight named ones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionColor {
    #[default]
    None,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

impl SessionColor {
    pub const ALL: [Self; 9] = [
        Self::None,
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::Teal,
        Self::Blue,
        Self::Purple,
        Self::Pink,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "No colour",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Teal => "Teal",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
        }
    }

    /// Its place among the theme's colours; `None` for none.
    pub(crate) const fn index(self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Red => Some(0),
            Self::Orange => Some(1),
            Self::Yellow => Some(2),
            Self::Green => Some(3),
            Self::Teal => Some(4),
            Self::Blue => Some(5),
            Self::Purple => Some(6),
            Self::Pink => Some(7),
        }
    }
}

/// The session dialog's "Colour": a swatch per colour, the chosen one ringed; each named for screen readers.
pub fn chooser(ui: &mut Ui, chosen: &mut SessionColor) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let p = theme::palette(ui);
        for color in SessionColor::ALL {
            let (rect, response) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
            let selected = *chosen == color;
            let painter = ui.painter();
            if let Some(fill) = theme::session_color(ui, color) {
                painter.circle_filled(rect.center(), 7.0, fill);
            } else {
                // No colour: an empty circle, struck through.
                painter.circle_stroke(rect.center(), 6.5, Stroke::new(1.0, p.muted));
                let slash = vec2(4.5, -4.5);
                painter.line_segment(
                    [rect.center() - slash, rect.center() + slash],
                    Stroke::new(1.0, p.muted),
                );
            }
            if selected || response.hovered() {
                let ring = if selected { p.text } else { p.border };
                painter.circle_stroke(rect.center(), 9.5, Stroke::new(1.5, ring));
            }
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, color.name())
            });
            if response.on_hover_text(color.name()).clicked() {
                *chosen = color;
            }
        }
    });
}

/// The 3 px line in the session's colour under the header (R-UI-35); nothing without a colour.
pub fn header_line(ui: &mut Ui, id: &str, color: Option<Color32>) {
    if let Some(color) = color {
        Panel::top(id)
            .exact_size(3.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::NONE.fill(color))
            .show(ui, |_| {});
    }
}

/// The 3 px stripe along the top of a tab drawn in `rect` (R-UI-35).
pub fn tab_stripe(ui: &Ui, rect: Rect, color: Option<Color32>) {
    if let Some(color) = color {
        let stripe = Rect::from_min_size(rect.min, vec2(rect.width(), 3.0));
        ui.painter().rect_filled(stripe, 1.0, color);
    }
}

/// The stripe along the left edge of a start screen card drawn in `rect` (R-UI-35).
pub fn card_stripe(ui: &Ui, rect: Rect, color: Option<Color32>) {
    if let Some(color) = color {
        let stripe = Rect::from_min_size(rect.min, vec2(4.0, rect.height()));
        ui.painter().rect_filled(stripe, 2.0, color);
    }
}

/// The block at the left end of the status bar (R-UI-35).
pub fn status_block(ui: &mut Ui, color: Option<Color32>) {
    if let Some(color) = color {
        let (rect, _) = ui.allocate_exact_size(vec2(4.0, 16.0), Sense::hover());
        ui.painter().rect_filled(rect, 2.0, color);
    }
}

/// The outline round the session pill drawn in `rect` (R-UI-35).
pub fn pill_outline(ui: &Ui, rect: Rect, color: Option<Color32>) {
    if let Some(color) = color {
        ui.painter().rect_stroke(
            rect.expand(1.0),
            6.0,
            Stroke::new(1.5, color),
            StrokeKind::Outside,
        );
    }
}

/// The 2 px frame round the screen, or a pane, drawn in `rect` (R-UI-35, R-SSH-61).
pub fn screen_frame(ui: &Ui, rect: Rect, color: Option<Color32>) {
    if let Some(color) = color {
        ui.painter()
            .rect_stroke(rect, 0.0, Stroke::new(2.0, color), StrokeKind::Inside);
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    #[test]
    fn every_colour_but_none_has_a_light_and_a_dark_variant_r_ui_35() {
        let mut seen = Vec::new();
        for dark in [false, true] {
            let mut harness = Harness::builder().build_ui_state(
                move |ui, shown: &mut Vec<Option<Color32>>| {
                    ui.ctx().set_visuals(if dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                    *shown = SessionColor::ALL
                        .iter()
                        .map(|&c| theme::session_color(ui, c))
                        .collect();
                },
                Vec::new(),
            );
            harness.run();
            let shown = harness.state().clone();
            assert_eq!(shown.first(), Some(&None), "no colour draws nothing");
            let colours: Vec<Color32> = shown.into_iter().flatten().collect();
            assert_eq!(colours.len(), 8);
            seen.push(colours);
        }
        for colours in &seen {
            let mut unique = colours.clone();
            unique.sort_by_key(Color32::to_array);
            unique.dedup();
            assert_eq!(unique.len(), 8, "eight different colours");
        }
        assert_ne!(seen[0], seen[1], "light and dark differ");
    }

    #[test]
    fn a_swatch_click_chooses_its_colour_r_ui_35() {
        let mut harness = Harness::builder().build_ui_state(
            |ui, chosen: &mut SessionColor| {
                crate::test_support::install_fonts(ui);
                chooser(ui, chosen);
            },
            SessionColor::None,
        );
        harness.get_by_label("Teal").click();
        harness.run();
        assert_eq!(*harness.state(), SessionColor::Teal);
        harness.get_by_label("No colour").click();
        harness.run();
        assert_eq!(*harness.state(), SessionColor::None);
    }
}
