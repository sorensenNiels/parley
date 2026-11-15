//! What both programs' Appearance pages share (R-UI-6, R-UI-8): the theme, the text size and
//! the density, and putting them into effect.

use egui::{ComboBox, Ui};
use serde::{Deserialize, Serialize};

use super::layout::group;
use crate::theme::{self, Density};

/// The text sizes offered, in percent.
const TEXT_SIZES: [u16; 7] = [80, 90, 100, 110, 125, 150, 175];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    /// Follow the operating system.
    #[default]
    System,
    Light,
    Dark,
}

/// How a program looks: what [`apply`] puts into effect. Each program's settings keep these
/// as fields of their own (stored settings are flat), and make a `Look` of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    pub theme: Theme,
    /// Everything's size, in percent.
    pub text_size: u16,
    pub density: Density,
    /// The terminal font's family name: a bundled font or an installed one.
    pub terminal_font: String,
}

/// Puts `new` into effect, changing only what differs from `old` (`None`: the first frame,
/// which installs the theme). Returns a note if the terminal font could not be loaded.
pub fn apply(ctx: &egui::Context, old: Option<&Look>, new: &Look) -> Option<String> {
    let note = match old {
        None => theme::install(ctx, &new.terminal_font),
        Some(old) if old.terminal_font != new.terminal_font => {
            theme::set_terminal_font(ctx, &new.terminal_font)
        }
        Some(_) => None,
    };
    if old.is_none_or(|o| o.density != new.density) {
        theme::set_sizes(ctx, new.density);
    }
    if old.is_none_or(|o| o.theme != new.theme) {
        ctx.set_theme(match new.theme {
            Theme::System => egui::ThemePreference::System,
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
        });
    }
    if old.is_none_or(|o| o.text_size != new.text_size) {
        ctx.set_zoom_factor(f32::from(new.text_size.clamp(50, 300)) / 100.0);
    }
    note
}

/// The Theme and Text groups of an Appearance page.
pub fn groups(ui: &mut Ui, theme_choice: &mut Theme, text_size: &mut u16, density: &mut Density) {
    group(ui, "Theme", None, |rows| {
        rows.row("Theme", None, |ui, _| {
            theme::segmented(
                ui,
                theme_choice,
                &[
                    (Theme::System, "Like the system"),
                    (Theme::Light, "Light"),
                    (Theme::Dark, "Dark"),
                ],
            );
        });
    });
    group(ui, "Text", None, |rows| {
        rows.row(
            "Text size",
            Some("Everything, the screen included."),
            |ui, name| {
                percent_choice(ui, text_size).labelled_by(name);
            },
        );
        rows.row(
            "Density",
            Some("Room around buttons and fields."),
            |ui, _| {
                theme::segmented(
                    ui,
                    density,
                    &[
                        (Density::Comfortable, "Comfortable"),
                        (Density::Compact, "Compact"),
                    ],
                );
            },
        );
    });
}

/// The text sizes, with `value` in the list even when it isn't one of them.
fn percent_choice(ui: &mut Ui, value: &mut u16) -> egui::Response {
    ComboBox::from_id_salt("text_size")
        .selected_text(format!("{value} %"))
        .show_ui(ui, |ui| {
            for size in text_sizes(*value) {
                ui.selectable_value(value, size, format!("{size} %"));
            }
        })
        .response
}

/// The sizes offered while the text size is `current`: the usual ones, and `current` in its
/// place when it is not one of them (R-UI-8).
fn text_sizes(current: u16) -> Vec<u16> {
    let mut sizes = TEXT_SIZES.to_vec();
    if !sizes.contains(&current) {
        sizes.push(current);
        sizes.sort_unstable();
    }
    sizes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of `ctx`: egui applies a new zoom when the next frame starts.
    fn frame(ctx: &egui::Context) {
        // The frame's output is not drawn anywhere: only the settings it applied matter.
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .textures_delta
            .clear();
    }

    fn look() -> Look {
        Look {
            theme: Theme::Dark,
            text_size: 125,
            density: Density::Compact,
            terminal_font: crate::fonts::DEFAULT_TERMINAL_FONT.to_owned(),
        }
    }

    /// R-UI-8: a text size from an older version, not in the list, is still offered in order.
    #[test]
    fn a_text_size_not_in_the_list_is_still_offered_r_ui_8() {
        assert_eq!(text_sizes(100), TEXT_SIZES.to_vec());
        assert_eq!(text_sizes(95), [80, 90, 95, 100, 110, 125, 150, 175]);
    }

    /// R-UI-6, R-UI-8: the first frame puts everything into effect; later ones only what changed.
    #[test]
    fn apply_puts_the_look_into_effect_and_then_only_what_changed_r_ui_6() {
        let ctx = egui::Context::default();
        let first = look();
        assert_eq!(apply(&ctx, None, &first), None);
        frame(&ctx);
        assert!((ctx.zoom_factor() - 1.25).abs() < f32::EPSILON);
        assert_eq!(
            ctx.options(|o| o.theme_preference),
            egui::ThemePreference::Dark
        );

        // Something else changed the zoom; an unchanged text size leaves it alone.
        ctx.set_zoom_factor(2.0);
        frame(&ctx);
        let lighter = Look {
            theme: Theme::Light,
            ..first.clone()
        };
        assert_eq!(apply(&ctx, Some(&first), &lighter), None);
        frame(&ctx);
        assert!((ctx.zoom_factor() - 2.0).abs() < f32::EPSILON);
        assert_eq!(
            ctx.options(|o| o.theme_preference),
            egui::ThemePreference::Light
        );
    }

    /// R-UI-8: a stored size far out of range is clamped, not applied as is.
    #[test]
    fn a_text_size_out_of_range_is_clamped_r_ui_8() {
        let ctx = egui::Context::default();
        let huge = Look {
            text_size: 1000,
            ..look()
        };
        apply(&ctx, None, &huge);
        frame(&ctx);
        assert!((ctx.zoom_factor() - 3.0).abs() < f32::EPSILON);
    }
}
