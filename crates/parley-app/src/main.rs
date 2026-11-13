//! Parley application entry point.
//!
//! Keep this crate thin: it opens the window and provides platform services.
//! UI code belongs in `parley-ui-egui`; the TN5250 engine in `parley-core`.

// Release builds on Windows are GUI apps: no console window opens next to Parley.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a binary reports to the terminal"
)]

mod desktop;
mod helpers;
mod keychain;
mod macros;
mod screenshot;

use parley_ui_egui::{ParleyUi, Profile};

/// Where the saved sessions are kept in eframe's storage.
const SESSIONS_KEY: &str = "sessions";
/// Where the settings (theme, size, terminal font) are kept.
const SETTINGS_KEY: &str = "settings";
/// Where Parley 0.1 kept its one connection's settings, read once to make the first session.
const OLD_CONFIG_KEY: &str = "connection";
/// Set once the passwords Parley 0.2 saved are deleted from the keychain (R-NET-7).
const PASSWORDS_REMOVED_KEY: &str = "passwords_removed";

/// Runs the UI inside eframe.
struct App {
    ui: ParleyUi,
    title: String,
    connect_on_start: Option<Profile>,
    screenshot: Option<screenshot::Pending>,
    /// The saved passwords are gone from the keychain: don't look again (R-NET-7).
    passwords_removed: bool,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(profile) = self.connect_on_start.take() {
            self.ui.connect(ui.ctx(), profile);
        }
        self.ui.ui(ui);
        if let Some(pending) = &mut self.screenshot {
            pending.update(ui.ctx());
        }
        let title = self.ui.title();
        if title != self.title {
            ui.ctx()
                .send_viewport_cmd(eframe::egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // A screenshot run is for development: it must not replace the operator's settings.
        if self.screenshot.is_some() {
            return;
        }
        eframe::set_value(storage, SESSIONS_KEY, &self.ui.profiles());
        eframe::set_value(storage, SETTINGS_KEY, self.ui.settings());
        if self.passwords_removed {
            eframe::set_value(storage, PASSWORDS_REMOVED_KEY, &true);
        }
    }
}

fn main() -> eframe::Result {
    desktop::run()
}
