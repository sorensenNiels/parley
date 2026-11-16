//! Parley SSH's own part of Settings ▸ Appearance (R-SSH-20, D49): the screen's themes for dark and light, with
//! previews, and its font, size, cursor and padding. Changes show at once.

use egui::{ComboBox, CornerRadius, Frame, Margin, RichText, Ui, Vec2};
use parley_ssh_core::term::CursorShape;
use parley_ui_egui::settings::group;
use parley_ui_egui::theme;
use serde::{Deserialize, Serialize};

use super::{FONT_SIZES, Settings};
use crate::screen::Palette;
use crate::screen::themes::TerminalTheme;

/// The cursor's shape as the settings keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    Block,
    Bar,
    Underline,
}

impl CursorStyle {
    pub const fn shape(self) -> CursorShape {
        match self {
            Self::Block => CursorShape::Block,
            Self::Bar => CursorShape::Bar,
            Self::Underline => CursorShape::Underline,
        }
    }
}

/// The Terminal theme and Terminal text groups.
pub(super) fn groups(
    ui: &mut Ui,
    settings: &mut Settings,
    installed: Option<&[String]>,
    font_note: Option<&str>,
) {
    group(
        ui,
        "Terminal theme",
        Some((
            "The screen's colours.",
            "The Theme above chooses which of the two the screen uses: When dark while Parley SSH is dark.",
        )),
        |rows| {
            rows.row("When dark", None, |ui, name| {
                theme_choice(ui, "ssh-dark-theme", &mut settings.dark_theme).labelled_by(name);
            });
            rows.row("When light", None, |ui, name| {
                theme_choice(ui, "ssh-light-theme", &mut settings.light_theme).labelled_by(name);
            });
            rows.full_row(|ui| {
                let dark = ui.visuals().dark_mode;
                preview(ui, &settings.terminal_theme(dark).palette());
            });
        },
    );
    group(ui, "Terminal text", None, |rows| {
        rows.row("Font", font_note, |ui, name| {
            parley_ui_egui::settings::font_choice(ui, &mut settings.terminal_font, installed)
                .labelled_by(name);
        });
        rows.row(
            "Size",
            Some("Ctrl+= and Ctrl+- in the terminal; Ctrl+0 back to 14."),
            |ui, name| {
                ui.add(
                    egui::DragValue::new(&mut settings.font_size)
                        .range(FONT_SIZES)
                        .suffix(" pt"),
                )
                .labelled_by(name);
            },
        );
        rows.row(
            "Cursor",
            Some("Until a program asks for another."),
            |ui, _| {
                theme::segmented(
                    ui,
                    &mut settings.cursor,
                    &[
                        (CursorStyle::Block, "Block"),
                        (CursorStyle::Bar, "Bar"),
                        (CursorStyle::Underline, "Underline"),
                    ],
                );
            },
        );
        rows.switch("Blinking cursor", None, &mut settings.cursor_blink);
        rows.row("Padding", Some("Room around the screen."), |ui, name| {
            ui.add(egui::Slider::new(&mut settings.padding, 0..=24).suffix(" pt"))
                .labelled_by(name);
        });
    });
}

/// A list of the themes, each with a strip of its colours.
fn theme_choice(ui: &mut Ui, id: &str, chosen: &mut TerminalTheme) -> egui::Response {
    ComboBox::from_id_salt(id)
        .selected_text(chosen.name())
        .width(220.0)
        .show_ui(ui, |ui| {
            for theme in TerminalTheme::ALL {
                ui.horizontal(|ui| {
                    swatch(ui, &theme.palette());
                    ui.selectable_value(chosen, theme, theme.name());
                });
            }
        })
        .response
}

/// The background with the six colours on it.
fn swatch(ui: &mut Ui, palette: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(56.0, 14.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), palette.background);
    for i in 1..=6_u8 {
        let x = rect.left() + 4.0 + f32::from(i - 1) * 8.0;
        let dot = egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), Vec2::splat(6.0));
        painter.rect_filled(dot, CornerRadius::same(1), palette.ansi(usize::from(i)));
    }
}

/// A few lines as a shell would show them, in the theme's colours.
fn preview(ui: &mut Ui, palette: &Palette) {
    let mono = |text: &str, colour| RichText::new(text).monospace().color(colour);
    Frame::new()
        .fill(palette.background)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(340.0);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
            ui.horizontal(|ui| {
                ui.label(mono("me@box", palette.ansi(2)));
                ui.label(mono(":", palette.foreground));
                ui.label(mono("~/src", palette.ansi(4)));
                ui.label(mono("$ ls", palette.foreground));
            });
            ui.horizontal(|ui| {
                ui.label(mono("build.sh  ", palette.ansi(2)));
                ui.label(mono("docs  ", palette.ansi(4)));
                ui.label(mono("notes.txt  ", palette.foreground));
                ui.label(mono("link", palette.ansi(6)));
            });
            ui.horizontal(|ui| {
                ui.label(mono("error ", palette.ansi(1)));
                ui.label(mono("warning ", palette.ansi(3)));
                ui.label(mono("info ", palette.ansi(5)));
                ui.label(mono("selected", palette.foreground).background_color(palette.selection));
                ui.label(mono(" ", palette.background).background_color(palette.cursor));
            });
        });
}
