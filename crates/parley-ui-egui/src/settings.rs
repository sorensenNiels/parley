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
use crate::groups::SessionGroup;
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent settings, each a switch in the dialog"
)]
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
    /// Keys typed while the host works are kept and carried out after (R-KEY-11).
    pub type_ahead: bool,
    /// The host's windows are drawn with frames (R-UI-26).
    pub window_frames: bool,
    /// How many screens each session keeps for the history view (R-UI-39).
    pub screens_kept: usize,
    /// What a line break does when pasting (R-KEY-12).
    pub paste_breaks: crate::keymap::PasteBreaks,
    /// The cursor's shape, and whether it blinks (R-UI-28).
    pub cursor: terminal::CursorShape,
    pub cursor_blink: bool,
    /// Look for a new version when Parley starts (R-UI-11).
    pub check_for_updates: bool,
    /// Which add-ons are on (R-ADD-1).
    pub addons: AddOnSettings,
    /// The operator's keyboard maps; the Default map is built in, not kept (R-KEY-2).
    pub keyboard_maps: Vec<KeyMap>,
    /// Where a duplicate and a group's sessions open (R-UI-34).
    pub more_sessions: MoreSessions,
    /// Named groups of saved sessions (R-UI-31).
    pub session_groups: Vec<SessionGroup>,
    /// The last connections made, newest first, at most [`crate::connect::RECENT_KEPT`]; one not saved has no name
    /// (R-UI-41).
    pub recent_sessions: Vec<crate::profile::Profile>,
    /// "All sessions" is open on the start screen (R-UI-42).
    pub all_sessions_open: bool,
}

/// Where more sessions open (R-UI-34, D53).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoreSessions {
    /// Another Parley window each, as before.
    #[default]
    Window,
    /// A tab each in this window (R-UI-33).
    Tab,
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
            type_ahead: false,
            window_frames: true,
            screens_kept: parley_core::session::SCREENS_KEPT,
            paste_breaks: crate::keymap::PasteBreaks::default(),
            cursor: terminal::CursorShape::default(),
            cursor_blink: false,
            check_for_updates: true,
            addons: AddOnSettings::default(),
            keyboard_maps: Vec::new(),
            more_sessions: MoreSessions::default(),
            session_groups: Vec::new(),
            recent_sessions: Vec::new(),
            all_sessions_open: false,
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
                settings.type_ahead = defaults.type_ahead;
                settings.window_frames = defaults.window_frames;
                settings.screens_kept = defaults.screens_kept;
                settings.cursor = defaults.cursor;
                settings.paste_breaks = defaults.paste_breaks;
                settings.cursor_blink = defaults.cursor_blink;
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

/// Said under Parley's add-ons while SSH is on (R-SSH-59).
pub(crate) const SSH_SETTINGS_ELSEWHERE: &str = "SSH… in the Tools menu starts Parley SSH \
     on the session's host, port 22. Its settings, saved sessions and keys are in Parley SSH.";

/// The open settings dialog.
#[derive(Debug)]
pub struct Dialog {
    /// The settings when the dialog opened, for Cancel.
    original: Settings,
    installed: Installed,
    /// Add-ons whose program is not installed, left out of the Add-ons page (R-NF-3).
    missing: Vec<AddOn>,
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
            missing: Vec::new(),
            page: Page::Appearance,
            keyboard: keyboard::Page::new(crate::keymap::DEFAULT_MAP, None, Vec::new()),
            about: about::About::default(),
        }
    }

    /// Shows the keyboard map `map` on the Keyboard page, with a key for each of `macros`
    /// (`None` while the Macros add-on is off) and for `tools`, the functions of the add-ons that are on.
    #[must_use]
    pub fn with_keyboard(
        mut self,
        map: &str,
        macros: Option<Vec<String>>,
        tools: Vec<crate::keymap::Function>,
    ) -> Self {
        self.keyboard = keyboard::Page::new(map, macros, tools);
        self
    }

    /// Keyboard maps renamed while the dialog was open, in order: (old name, new name). Only
    /// for a dialog closed with Save.
    pub fn take_map_renames(&mut self) -> Vec<(String, String)> {
        self.keyboard.take_renames()
    }

    /// Leaves `missing`, add-ons whose program is not installed, out of the Add-ons page (R-NF-3).
    #[must_use]
    pub fn without_addons(mut self, missing: Vec<AddOn>) -> Self {
        self.missing = missing;
        self
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
            Page::AddOns => Self::addons_page(ui, settings, &self.missing),
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

    /// A switch per add-on (R-ADD-1), as Tortuga's Settings → Add-ons; while SQL or SSH is on,
    /// where its settings are (R-SQL-69, R-SSH-59).
    fn addons_page(ui: &mut Ui, settings: &mut Settings, missing: &[AddOn]) {
        let note = (
            "An add-on that is off is not loaded. Changes take effect when you save.",
            "Add-ons are parts of Parley not everyone needs. Turning one on adds its menu items; \
             turning it off removes them and stops what it is doing.",
        );
        group(ui, "Add-ons", Some(note), |rows| {
            for addon in AddOn::ALL.into_iter().filter(|a| !missing.contains(a)) {
                let mut on = settings.addons.enabled(addon);
                rows.switch(addon.name(), Some(addon.about()), &mut on);
                settings.addons.set_enabled(addon, on);
            }
        });
        if settings.addons.enabled(AddOn::Sql) && !missing.contains(&AddOn::Sql) {
            theme::hint(ui, SQL_SETTINGS_ELSEWHERE);
        }
        if settings.addons.enabled(AddOn::Ssh) && !missing.contains(&AddOn::Ssh) {
            theme::hint(ui, SSH_SETTINGS_ELSEWHERE);
        }
    }

    fn terminal_page(&self, ui: &mut Ui, settings: &mut Settings) {
        sessions_group(ui, settings);
        let installed = self.installed.get();
        Self::terminal_rest(ui, settings, installed.as_deref());
    }

    fn terminal_rest(ui: &mut Ui, settings: &mut Settings, installed: Option<&[String]>) {
        group(ui, "Font", None, |rows| {
            rows.row(
                "Font",
                Some("A bundled font, or a monospace font on this computer."),
                |ui, name| {
                    font_choice(ui, &mut settings.terminal_font, installed).labelled_by(name);
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
            rows.row(
                "Cursor",
                Some("In insert mode it shows as a bar, or a block when this is the bar."),
                |ui, _| {
                    theme::segmented(ui, &mut settings.cursor, &terminal::CursorShape::ALL);
                },
            );
            rows.switch("Blinking cursor", None, &mut settings.cursor_blink);
            rows.row(
                "Paste line breaks",
                Some("Next row: each pasted line goes in one row down, at the column it began."),
                |ui, _| {
                    use crate::keymap::PasteBreaks;
                    theme::segmented(
                        ui,
                        &mut settings.paste_breaks,
                        &[
                            (PasteBreaks::NextField, "Next field"),
                            (PasteBreaks::NextRow, "Next row"),
                        ],
                    );
                },
            );
            rows.switch(
                "Window frames",
                Some("Draw the IBM i's pop-up windows with a frame instead of dots and colons."),
                &mut settings.window_frames,
            );
            rows.switch(
                "Type-ahead",
                Some("Keys typed while the IBM i works (X SYSTEM) are kept and typed when it is done."),
                &mut settings.type_ahead,
            );
            rows.row(
                "Screens kept",
                Some("For the screen history (Ctrl+Page Up), in memory while the session's tab is open."),
                |ui, name| {
                    ui.add(
                        egui::DragValue::new(&mut settings.screens_kept)
                            .range(parley_core::session::SCREENS_KEPT_RANGE),
                    )
                    .labelled_by(name);
                },
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

/// The bundled fonts, then the installed monospace ones (`None` while still looking). Parley SSH's Appearance page
/// uses it too (R-SSH-20).
pub fn font_choice(ui: &mut Ui, font: &mut String, installed: Option<&[String]>) -> egui::Response {
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

/// Where more sessions open (R-UI-34).
fn sessions_group(ui: &mut Ui, settings: &mut Settings) {
    group(ui, "Sessions", None, |rows| {
        rows.row(
            "Open more sessions in",
            Some("Where Duplicate session and a group's sessions open."),
            |ui, _| {
                theme::segmented(
                    ui,
                    &mut settings.more_sessions,
                    &[
                        (MoreSessions::Window, "A new window"),
                        (MoreSessions::Tab, "A new tab"),
                    ],
                );
            },
        );
    });
}
