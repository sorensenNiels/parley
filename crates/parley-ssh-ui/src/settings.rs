//! Parley SSH's settings (R-SSH-10), in its own storage: the look shared with Parley 5250 and Parley SQL
//! (Appearance and About are Parley's pages), the last connection on the start screen, and a few choices of its
//! own. Changes show at once; Cancel puts the old ones back.

use egui::Ui;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

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
        }
    }
}

impl Settings {
    /// How Parley SSH looks: the UI's look, and Parley's default terminal font for the screen (R-SSH-8).
    pub fn look(&self) -> Look {
        Look {
            theme: self.theme,
            text_size: self.text_size,
            density: self.density,
            terminal_font: parley_ui_egui::fonts::DEFAULT_TERMINAL_FONT.to_owned(),
        }
    }
}

/// A page of the dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Appearance,
    Terminal,
    About,
}

impl SettingsPage for Page {
    fn name(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Terminal => "Terminal",
            Self::About => "About",
        }
    }
}

const SECTIONS: Sections<Page> = &[
    ("General", &[Page::Appearance, Page::Terminal]),
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
            }
            Self::Terminal => {
                settings.confirm_close = defaults.confirm_close;
                settings.confirm_paste = defaults.confirm_paste;
            }
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
}

impl Dialog {
    pub fn open(settings: &Settings) -> Self {
        Self {
            original: settings.clone(),
            page: Page::Appearance,
            about: About::default(),
        }
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
            }
            Page::Terminal => terminal_page(ui, settings),
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
}
