//! Parley's settings (R-UI-6, R-UI-8, R-UI-11, R-KEY-2, R-ADD-1): theme, text size, density,
//! the terminal font, keyboard maps, add-ons and looking for new versions, with what this build
//! is on the About page, in a dialog from the gear in the header (R-UI-18). Parley SQL's
//! settings are its own (`parley-sql-ui`, R-SQL-69); the dialog's frame, its Appearance groups
//! and About page are shared with it ([`layout`], [`appearance`], [`about`]).
//! Changes show at once; Cancel puts the old ones back.

pub mod about;
pub mod appearance;
pub mod key_capture;
mod keyboard;
pub mod layout;

use egui::{ComboBox, CornerRadius, Frame, Margin, RichText, Ui};
use serde::{Deserialize, Serialize};

pub use appearance::{Look, Theme, apply};
pub use layout::{Clicked, Closed, Rows, Sections, SettingsPage, group};

use crate::addons::{AddOn, AddOnSettings, Toggle};
use crate::fonts::{self, Installed};
use crate::keymap::KeyMap;
use crate::menu::Program;
use crate::terminal;
use crate::theme::{self, Density};
use crate::updates::UpdateStatus;

/// Parley's settings, kept by the launcher. Stored flat: a field's name is its key in the
/// stored settings, and fields Parley no longer has (the SQL window's, before R-SQL-70) are
/// ignored when read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// Everything's size, in percent.
    #[serde(alias = "zoom")]
    pub text_size: u16,
    pub density: Density,
    /// The terminal font's family name: a bundled font or an installed one.
    pub terminal_font: String,
    /// The 5250 screen's colours, unless a session chooses its own (R-UI-23).
    pub screen_colors: terminal::Scheme,
    /// The colours of the Custom scheme (R-UI-23).
    pub custom_colors: terminal::CustomColors,
    /// What the host's alarm does (R-UI-25).
    pub alarm: Alarm,
    /// Blinking text blinks; off, it shows steady in reverse image (R-UI-24).
    pub blinking_text: bool,
    /// Look for a new version when Parley starts (R-UI-11).
    pub check_for_updates: bool,
    /// Which add-ons are on (R-ADD-1).
    pub addons: AddOnSettings,
    /// The operator's keyboard maps; the Default map is built in, not kept (R-KEY-2).
    pub keyboard_maps: Vec<KeyMap>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            text_size: 100,
            density: Density::Comfortable,
            terminal_font: fonts::DEFAULT_TERMINAL_FONT.to_owned(),
            screen_colors: terminal::Scheme::default(),
            custom_colors: terminal::CustomColors::default(),
            alarm: Alarm::default(),
            blinking_text: true,
            check_for_updates: true,
            addons: AddOnSettings::default(),
            keyboard_maps: Vec::new(),
        }
    }
}

impl Settings {
    /// How Parley looks with these settings.
    pub fn look(&self) -> Look {
        Look {
            theme: self.theme,
            text_size: self.text_size,
            density: self.density,
            terminal_font: self.terminal_font.clone(),
        }
    }
}

/// What the host's alarm does (R-UI-25).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alarm {
    #[default]
    SoundAndFlash,
    FlashOnly,
    Off,
}

/// A group of settings: one entry in the dialog's list on the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Appearance,
    Terminal,
    Keyboard,
    AddOns,
    About,
}

impl SettingsPage for Page {
    fn name(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Terminal => "Terminal",
            Self::Keyboard => "Keyboard",
            Self::AddOns => "Add-ons",
            Self::About => "About",
        }
    }
}

impl Page {
    /// Puts this page's settings back to their defaults; the other pages keep theirs.
    fn restore_defaults(self, settings: &mut Settings) {
        let defaults = Settings::default();
        match self {
            Self::Appearance => {
                settings.theme = defaults.theme;
                settings.text_size = defaults.text_size;
                settings.density = defaults.density;
            }
            Self::Terminal => {
                settings.terminal_font = defaults.terminal_font;
                settings.screen_colors = defaults.screen_colors;
                settings.custom_colors = defaults.custom_colors;
                settings.alarm = defaults.alarm;
                settings.blinking_text = defaults.blinking_text;
            }
            // The map shown gets the Default map's keys: see `keyboard::Page::restore`.
            Self::Keyboard => {}
            Self::AddOns => {
                for addon in AddOn::ALL {
                    settings.addons.set_enabled(addon, addon.on_by_default());
                }
            }
            Self::About => settings.check_for_updates = defaults.check_for_updates,
        }
    }
}

/// Parley's pages: none of Parley SQL's, which has its own (R-SQL-69).
const SECTIONS: Sections<Page> = &[
    (
        "General",
        &[Page::Appearance, Page::Terminal, Page::Keyboard],
    ),
    ("App", &[Page::AddOns, Page::About]),
];

/// Said under Parley's add-ons while SQL is on (R-SQL-69, R-SQL-70).
pub(crate) const SQL_SETTINGS_ELSEWHERE: &str = "SQL… in the Tools menu starts Parley SQL \
     on the session's host. Its settings, such as Java and the SQL assistant, are in Parley SQL.";

/// The open settings dialog.
#[derive(Debug)]
pub struct Dialog {
    /// The settings when the dialog opened, for Cancel.
    original: Settings,
    installed: Installed,
    page: Page,
    keyboard: keyboard::Page,
    about: about::About,
}

impl Dialog {
    pub fn open(ctx: &egui::Context, settings: &Settings) -> Self {
        let ctx = ctx.clone();
        Self {
            original: settings.clone(),
            installed: Installed::search(move || ctx.request_repaint()),
            page: Page::Appearance,
            keyboard: keyboard::Page::new(crate::keymap::DEFAULT_MAP, None, false),
            about: about::About::default(),
        }
    }

    /// Shows the keyboard map `map` on the Keyboard page, with a key for each of `macros`
    /// (`None` while the Macros add-on is off).
    #[must_use]
    pub fn with_keyboard(mut self, map: &str, macros: Option<Vec<String>>, sql: bool) -> Self {
        self.keyboard = keyboard::Page::new(map, macros, sql);
        self
    }

    /// Keyboard maps renamed while the dialog was open, in order: (old name, new name). Only
    /// for a dialog closed with Save.
    pub fn take_map_renames(&mut self) -> Vec<(String, String)> {
        self.keyboard.take_renames()
    }

    /// The same dialog on the Keyboard page ("Change keys…" in the shortcut sheet, R-UI-17).
    #[must_use]
    pub fn on_keyboard_page(mut self) -> Self {
        self.page = Page::Keyboard;
        self
    }

    /// True once after "Check now" was clicked.
    pub fn take_check_request(&mut self) -> bool {
        self.about.take_check_request()
    }

    pub fn original(&self) -> &Settings {
        &self.original
    }

    /// Draws the dialog over `settings`, which change as the operator picks.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &mut Settings,
        updates: &UpdateStatus,
    ) -> Option<Closed> {
        let mut page = self.page;
        let clicked = layout::frame(ctx, &mut page, SECTIONS, |ui, page| {
            self.page_content(ui, page, settings, updates);
        });
        self.page = page;
        match clicked? {
            Clicked::Keep => Some(Closed::Keep),
            Clicked::Revert => Some(Closed::Revert),
            Clicked::RestoreDefaults => {
                if self.page == Page::Keyboard {
                    self.keyboard.restore(settings);
                } else {
                    self.page.restore_defaults(settings);
                }
                None
            }
        }
    }

    fn page_content(
        &mut self,
        ui: &mut Ui,
        page: Page,
        settings: &mut Settings,
        updates: &UpdateStatus,
    ) {
        match page {
            Page::Appearance => appearance::groups(
                ui,
                &mut settings.theme,
                &mut settings.text_size,
                &mut settings.density,
            ),
            Page::Terminal => self.terminal_page(ui, settings),
            Page::Keyboard => self.keyboard.show(ui, settings),
            Page::AddOns => Self::addons_page(ui, settings),
            Page::About => {
                self.about.show(
                    ui,
                    Program::Parley,
                    &mut settings.check_for_updates,
                    updates,
                );
            }
        }
    }

    /// A switch per add-on (R-ADD-1), as Tortuga's Settings → Add-ons; while SQL is on, where
    /// its settings are (R-SQL-69).
    fn addons_page(ui: &mut Ui, settings: &mut Settings) {
        let note = (
            "An add-on that is off is not loaded. Changes take effect when you save.",
            "Add-ons are parts of Parley not everyone needs. Turning one on adds its menu items; \
             turning it off removes them and stops what it is doing.",
        );
        group(ui, "Add-ons", Some(note), |rows| {
            for addon in AddOn::ALL {
                let mut on = settings.addons.enabled(addon);
                rows.switch(addon.name(), Some(addon.about()), &mut on);
                settings.addons.set_enabled(addon, on);
            }
        });
        if settings.addons.enabled(AddOn::Sql) {
            theme::hint(ui, SQL_SETTINGS_ELSEWHERE);
        }
    }

    fn terminal_page(&self, ui: &mut Ui, settings: &mut Settings) {
        let installed = self.installed.get();
        group(ui, "Font", None, |rows| {
            rows.row(
                "Font",
                Some("A bundled font, or a monospace font on this computer."),
                |ui, name| {
                    font_choice(ui, &mut settings.terminal_font, installed.as_deref())
                        .labelled_by(name);
                },
            );
        });
        group(ui, "Colours", None, |rows| {
            rows.row(
                "Colours",
                Some("A session can have its own, in the session dialog."),
                |ui, _| {
                    theme::segmented(ui, &mut settings.screen_colors, &terminal::Scheme::ALL);
                },
            );
            if settings.screen_colors == terminal::Scheme::Custom {
                rows.row(
                    "Custom colours",
                    Some("The background, then the seven 5250 colours."),
                    |ui, _| custom_colors(ui, &mut settings.custom_colors),
                );
            }
            rows.row(
                "Alarm",
                Some("When the IBM i sounds the alarm, usually with an error."),
                |ui, _| {
                    theme::segmented(
                        ui,
                        &mut settings.alarm,
                        &[
                            (Alarm::SoundAndFlash, "Sound and flash"),
                            (Alarm::FlashOnly, "Flash"),
                            (Alarm::Off, "Off"),
                        ],
                    );
                },
            );
            rows.switch(
                "Blinking text",
                Some("Off: blinking text shows steady in reverse image."),
                &mut settings.blinking_text,
            );
            rows.full_row(|ui| {
                let palette =
                    terminal::Palette::of(settings.screen_colors, &settings.custom_colors);
                preview(ui, &palette);
                theme::hint(
                    ui,
                    "The font's size follows the window: the screen always fills it.",
                );
            });
        });
    }
}

/// The bundled fonts, then the installed monospace ones (`None` while still looking).
fn font_choice(ui: &mut Ui, font: &mut String, installed: Option<&[String]>) -> egui::Response {
    ComboBox::from_id_salt("terminal_font")
        .selected_text(font.as_str())
        .width(240.0)
        .show_ui(ui, |ui| {
            theme::section_label(ui, "Included with Parley");
            for name in fonts::BUNDLED {
                ui.selectable_value(font, name.to_owned(), name);
            }
            ui.separator();
            theme::section_label(ui, "Installed");
            match installed {
                None => {
                    ui.spinner();
                }
                Some([]) => theme::hint(ui, "No other monospace fonts found."),
                Some(names) => {
                    for name in names {
                        ui.selectable_value(font, name.clone(), name);
                    }
                }
            }
        })
        .response
}

/// A colour button for the background and for each 5250 colour, named for screen readers.
fn custom_colors(ui: &mut Ui, custom: &mut terminal::CustomColors) {
    ui.horizontal(|ui| {
        let named = |ui: &mut Ui, rgb: &mut [u8; 3], name: &str| {
            let button = egui::color_picker::color_edit_button_srgb(ui, rgb).on_hover_text(name);
            button.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
        };
        named(ui, &mut custom.background, "Background");
        for (rgb, (_, name)) in custom.colors.iter_mut().zip(terminal::COLORS) {
            named(ui, rgb, name);
        }
    });
}

/// A few lines of a 5250 screen in the terminal font and `palette`, with every colour named.
fn preview(ui: &mut Ui, palette: &terminal::Palette) {
    let font = fonts::terminal_font(ui.ctx(), 15.0);
    Frame::new()
        .fill(palette.background)
        .inner_margin(Margin::same(10))
        .corner_radius(CornerRadius::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            let line = |ui: &mut Ui, text: &str, color| {
                ui.label(RichText::new(text).font(font.clone()).color(color));
            };
            let white = palette.color(parley_core::Color::White);
            let green = palette.color(parley_core::Color::Green);
            line(ui, "            Sign On             ", white);
            line(ui, "System . . . . . :   PUB400     ", green);
            line(ui, "User . . . . . . . . QSECOFR    ", green);
            line(ui, "0Oo 1lI| {}[] æøå ÆØÅ €  ", green);
            ui.horizontal_wrapped(|ui| {
                for (color, name) in terminal::COLORS {
                    line(ui, name, palette.color(color));
                }
            });
        });
    ui.add_space(4.0);
}
