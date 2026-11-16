//! Parley SSH's settings (R-SSH-10), in its own storage: the look shared with Parley 5250 and Parley SQL
//! (Appearance and About are Parley's pages), the last connection on the start screen, and a few choices of its
//! own. Changes show at once; Cancel puts the old ones back.

mod appearance;

use egui::Ui;
use serde::{Deserialize, Serialize};

pub use appearance::CursorStyle;

use crate::screen::themes::TerminalTheme;

use parley_ui_egui::menu::Program;
use parley_ui_egui::settings::about::About;
use parley_ui_egui::settings::{
    self as shared, Clicked, Look, Sections, SettingsPage, Theme, group,
};
use parley_ui_egui::theme::Density;
use parley_ui_egui::updates::UpdateStatus;

pub use parley_ui_egui::settings::Closed;
pub use shared::apply;

/// Parley SSH's settings, kept by the launcher.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent on/off settings, as the settings file keeps them"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    /// Everything's size, the screen's too, in percent.
    pub text_size: u16,
    pub density: Density,
    /// Look for a new version when Parley SSH starts (R-UI-11, D13).
    pub check_for_updates: bool,
    /// Closing the window while connected asks first (R-SSH-10).
    pub confirm_close: bool,
    /// Pasting several lines outside bracketed paste asks first (R-SSH-9).
    pub confirm_paste: bool,
    /// The start screen's quick connect: the last host, port and user (R-SSH-2).
    pub last_host: String,
    pub last_port: u16,
    pub last_user: String,
    /// The saved sessions, in the order they were made (R-SSH-11).
    pub sessions: Vec<crate::sessions::SavedSession>,
    /// What a new session, and the quick connect, start with (`PuTTY`'s "Default Settings").
    pub defaults: crate::sessions::SavedSession,
    /// The screen's font, bundled or installed (R-SSH-20).
    pub terminal_font: String,
    /// The screen's text size in points at 100 % (8–32; Ctrl+= / Ctrl+- / Ctrl+0).
    pub font_size: u8,
    /// The screen's theme while the UI is dark, and while it is light (D49).
    pub dark_theme: TerminalTheme,
    pub light_theme: TerminalTheme,
    /// The cursor until a program asks for another (R-SSH-20).
    pub cursor: CursorStyle,
    pub cursor_blink: bool,
    /// Room around the screen, in points (0–24).
    pub padding: u8,
    /// Commands kept by name (R-SSH-49, D50).
    pub snippets: Vec<crate::snippets::Snippet>,
    /// A lost connection reconnects by itself, for this many minutes (R-SSH-51).
    pub reconnect: bool,
    pub reconnect_minutes: u8,
    /// Open the tabs of last time on start (R-SSH-52, off by default), and those tabs.
    pub restore_tabs: bool,
    pub last_tabs: Vec<crate::restore::SavedTab>,
}

/// The screen's text sizes (R-SSH-20).
pub const FONT_SIZES: std::ops::RangeInclusive<u8> = 8..=32;
pub const DEFAULT_FONT_SIZE: u8 = 14;

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            text_size: 100,
            density: Density::Comfortable,
            check_for_updates: true,
            confirm_close: true,
            confirm_paste: true,
            last_host: String::new(),
            last_port: 22,
            last_user: String::new(),
            sessions: Vec::new(),
            defaults: crate::sessions::SavedSession::default(),
            terminal_font: parley_ui_egui::fonts::DEFAULT_TERMINAL_FONT.to_owned(),
            font_size: DEFAULT_FONT_SIZE,
            dark_theme: TerminalTheme::ParleyDark,
            light_theme: TerminalTheme::ParleyLight,
            cursor: CursorStyle::Block,
            cursor_blink: true,
            padding: 6,
            snippets: Vec::new(),
            reconnect: true,
            reconnect_minutes: 10,
            restore_tabs: false,
            last_tabs: Vec::new(),
        }
    }
}

impl Settings {
    /// How Parley SSH looks: the UI's look, and the screen's font (R-SSH-20).
    pub fn look(&self) -> Look {
        Look {
            theme: self.theme,
            text_size: self.text_size,
            density: self.density,
            terminal_font: self.terminal_font.clone(),
        }
    }

    /// The screen's theme for the UI's dark or light mode (D49).
    pub const fn terminal_theme(&self, dark: bool) -> TerminalTheme {
        if dark {
            self.dark_theme
        } else {
            self.light_theme
        }
    }

    /// The screen's look in the UI's dark or light mode (R-SSH-20).
    pub fn screen_look(&self, dark: bool) -> crate::screen::Look {
        crate::screen::Look {
            palette: self.terminal_theme(dark).palette(),
            font_size: f32::from(self.font_size.clamp(*FONT_SIZES.start(), *FONT_SIZES.end())),
            padding: f32::from(self.padding.min(24)),
            cursor: self.cursor.shape(),
            cursor_blink: self.cursor_blink,
        }
    }

    /// One step bigger (`1`), smaller (`-1`), or back to the default (`0`), as Ctrl+= / Ctrl+- / Ctrl+0.
    pub fn step_font_size(&mut self, step: i8) {
        self.font_size = match step {
            0 => DEFAULT_FONT_SIZE,
            s => self
                .font_size
                .saturating_add_signed(s)
                .clamp(*FONT_SIZES.start(), *FONT_SIZES.end()),
        };
    }
}

/// A page of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Appearance,
    Terminal,
    Snippets,
    About,
}

impl SettingsPage for Page {
    fn name(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Terminal => "Terminal",
            Self::Snippets => "Snippets",
            Self::About => "About",
        }
    }
}

const SECTIONS: Sections<Page> = &[
    (
        "General",
        &[Page::Appearance, Page::Terminal, Page::Snippets],
    ),
    ("App", &[Page::About]),
];

impl Page {
    fn restore_defaults(self, settings: &mut Settings) {
        let defaults = Settings::default();
        match self {
            Self::Appearance => {
                settings.theme = defaults.theme;
                settings.text_size = defaults.text_size;
                settings.density = defaults.density;
                settings.terminal_font = defaults.terminal_font;
                settings.font_size = defaults.font_size;
                settings.dark_theme = defaults.dark_theme;
                settings.light_theme = defaults.light_theme;
                settings.cursor = defaults.cursor;
                settings.cursor_blink = defaults.cursor_blink;
                settings.padding = defaults.padding;
            }
            Self::Terminal => {
                settings.confirm_close = defaults.confirm_close;
                settings.confirm_paste = defaults.confirm_paste;
                settings.reconnect = defaults.reconnect;
                settings.reconnect_minutes = defaults.reconnect_minutes;
                settings.restore_tabs = defaults.restore_tabs;
            }
            // The snippets are the user's own: Restore defaults leaves them.
            Self::Snippets => {}
            Self::About => settings.check_for_updates = defaults.check_for_updates,
        }
    }
}

/// The open settings dialog.
#[derive(Debug)]
pub struct Dialog {
    original: Settings,
    page: Page,
    about: About,
    /// The installed monospace fonts, looked for when the dialog opens.
    installed: parley_ui_egui::fonts::Installed,
    /// Said when the chosen font is not installed.
    pub font_note: Option<String>,
    snippets: crate::snippets::Page,
}

impl Dialog {
    pub fn open(ctx: &egui::Context, settings: &Settings) -> Self {
        let ctx = ctx.clone();
        Self {
            original: settings.clone(),
            page: Page::Appearance,
            about: About::default(),
            installed: parley_ui_egui::fonts::Installed::search(move || ctx.request_repaint()),
            font_note: None,
            snippets: crate::snippets::Page::default(),
        }
    }

    /// Opens on `page`.
    #[must_use]
    pub const fn on(mut self, page: Page) -> Self {
        self.page = page;
        self
    }

    pub fn original(&self) -> &Settings {
        &self.original
    }

    /// True once after "Check now" was clicked.
    pub fn take_check_request(&mut self) -> bool {
        self.about.take_check_request()
    }

    /// Draws the dialog over `settings`, which change as the user picks.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &mut Settings,
        updates: &UpdateStatus,
        files: Option<&dyn crate::snippets::SnippetFiles>,
    ) -> Option<Closed> {
        let mut page = self.page;
        let clicked = shared::layout::frame(ctx, &mut page, SECTIONS, |ui, page| match page {
            Page::Appearance => {
                shared::appearance::groups(
                    ui,
                    &mut settings.theme,
                    &mut settings.text_size,
                    &mut settings.density,
                );
                let installed = self.installed.get();
                appearance::groups(
                    ui,
                    settings,
                    installed.as_deref(),
                    self.font_note.as_deref(),
                );
            }
            Page::Terminal => terminal_page(ui, settings),
            Page::Snippets => self.snippets.show(ui, &mut settings.snippets, files),
            Page::About => self.about.show(
                ui,
                Program::ParleySsh,
                &mut settings.check_for_updates,
                updates,
            ),
        });
        self.page = page;
        match clicked? {
            Clicked::Keep => Some(Closed::Keep),
            Clicked::Revert => Some(Closed::Revert),
            Clicked::RestoreDefaults => {
                self.page.restore_defaults(settings);
                None
            }
        }
    }
}

fn terminal_page(ui: &mut Ui, settings: &mut Settings) {
    group(ui, "Terminal", None, |rows| {
        rows.switch(
            "Ask before closing",
            Some("While a session is connected."),
            &mut settings.confirm_close,
        );
        rows.switch(
            "Ask before pasting several lines",
            Some("Each line may run as a command, unless the program asked for bracketed paste."),
            &mut settings.confirm_paste,
        );
    });
    group(ui, "Connections", None, |rows| {
        rows.switch(
            "Reconnect a lost connection",
            Some("After sleep or a network change; a password is asked again."),
            &mut settings.reconnect,
        );
        if settings.reconnect {
            rows.row("Keep trying for", None, |ui, name| {
                ui.add(
                    egui::DragValue::new(&mut settings.reconnect_minutes)
                        .range(1..=60)
                        .suffix(" min"),
                )
                .labelled_by(name);
            });
        }
        rows.switch(
            "Open the tabs of last time",
            Some("On start, with their split panes and sessions; never a password."),
            &mut settings.restore_tabs,
        );
    });
}
