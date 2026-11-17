//! Parley SSH (D41, R-SSH-1): an SSH client in Parley's look.
//!
//! Thin like `parley` and `parley-sql`: it opens the window and hands over the platform services from this
//! crate's library. The UI is `parley_ssh_ui::ParleySshUi`; the command line is `ssh_start` (R-SSH-14).

// Release builds on Windows are GUI apps: no console window opens next to Parley SSH.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a binary reports to the terminal"
)]

use eframe::egui;
use parley_app::screenshot;
use parley_app::ssh_start::{self, Command};
use parley_ssh_core::known_hosts::KnownHosts;
use parley_ssh_ui::{ParleySshUi, Settings, StartRequest};

/// Parley SSH's app id: its settings folder, apart from Parley's and Parley SQL's.
const APP_ID: &str = "parley-ssh";
/// Where the settings are kept in eframe's storage.
const SETTINGS_KEY: &str = "settings";

struct App {
    ui: ParleySshUi,
    /// What the command line asked to open, on the first frame (R-SSH-14).
    start: Option<StartRequest>,
    title: String,
    screenshot: Option<screenshot::Pending>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(request) = self.start.take() {
            self.ui.start(ui.ctx(), request);
        }
        self.ui.ui(ui);
        if let Some(pending) = &mut self.screenshot {
            pending.update(ui.ctx());
        }
        let title = self.ui.title();
        if title != self.title {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // A screenshot run is for development: it must not replace the user's settings.
        if self.screenshot.is_none() {
            // With the tabs open now, when restore is on (R-SSH-52).
            eframe::set_value(storage, SETTINGS_KEY, &self.ui.settings_to_save());
        }
    }
}

fn main() -> eframe::Result {
    let (request, screenshot) = match ssh_start::parse(std::env::args().skip(1)) {
        Ok(Command::Run {
            request,
            screenshot,
        }) => (request, screenshot),
        Ok(Command::Version) => {
            println!("parley-ssh {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Ok(Command::Help) => {
            println!("{}", ssh_start::USAGE);
            return Ok(());
        }
        Err(e) => {
            eprintln!("parley-ssh: {e}\n\n{}", ssh_start::USAGE);
            std::process::exit(2);
        }
    };
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Parley SSH")
        .with_app_id(APP_ID)
        .with_inner_size([960.0, 640.0])
        .with_min_inner_size([480.0, 320.0]);
    match parley_app::window_icon() {
        Ok(icon) => viewport = viewport.with_icon(std::sync::Arc::new(icon)),
        Err(e) => eprintln!("parley-ssh: could not load the window icon: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    // Host keys beside the settings (R-SSH-3); the current folder only if the system names no data folder.
    let folder = eframe::storage_dir(APP_ID).unwrap_or_default();
    let known_hosts = KnownHosts::in_folder(&folder);
    eframe::run_native(
        "Parley SSH",
        options,
        Box::new(move |cc| {
            let settings: Settings = cc
                .storage
                .and_then(|s| eframe::get_value(s, SETTINGS_KEY))
                .unwrap_or_default();
            let mut ui = ParleySshUi::new(settings, known_hosts);
            ui.set_update_checker(parley_app::update_checker());
            ui.set_spawner(std::sync::Arc::new(ssh_start::spawn));
            ui.set_snippet_files(Box::new(parley_app::helpers::SnippetFileWindow));
            ui.set_notifier(std::sync::Arc::new(notify));
            ui.set_beeper(std::sync::Arc::new(parley_app::beep::beep));
            // The hosts of ~/.ssh/config (R-SSH-42); none without a home folder.
            if let Some(config) = parley_ssh_core::openssh_config::Config::from_home() {
                ui.set_openssh_config(config);
            }
            match parley_app::logo() {
                Ok(logo) => ui.set_logo(logo),
                Err(e) => eprintln!("parley-ssh: could not load the menu bar icon: {e}"),
            }
            Ok(Box::new(App {
                ui,
                start: Some(request),
                title: String::new(),
                screenshot: screenshot.map(screenshot::Pending::new),
            }))
        }),
    )
}

/// A desktop notification (R-SSH-46), on a thread of its own: D-Bus may take a moment and must not hold up the
/// window. A system without notifications loses only the notice, so a failure is not said.
fn notify(title: &str, body: &str) {
    let (title, body) = (title.to_owned(), body.to_owned());
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .appname("Parley SSH")
            .summary(&title)
            .body(&body)
            .show();
    });
}
