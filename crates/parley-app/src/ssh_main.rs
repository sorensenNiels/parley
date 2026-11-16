//! Parley SSH (D41, R-SSH-1): an SSH client in Parley's look.
//!
//! Thin like `parley` and `parley-sql`: it opens the window and hands over the platform services from this
//! crate's library. The UI is `parley_ssh_ui::ParleySshUi`. The full command line comes with R-SSH-14.

// Release builds on Windows are GUI apps: no console window opens next to Parley SSH.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a binary reports to the terminal"
)]

use eframe::egui;
use parley_app::screenshot;
use parley_ssh_core::known_hosts::KnownHosts;
use parley_ssh_ui::{ParleySshUi, Settings};

/// Parley SSH's app id: its settings folder, apart from Parley's and Parley SQL's.
const APP_ID: &str = "parley-ssh";
/// Where the settings are kept in eframe's storage.
const SETTINGS_KEY: &str = "settings";

const USAGE: &str = "\
Usage: parley-ssh [OPTION]

Opens Parley SSH, an SSH client.

Options:
  --screenshot FILE    After a few seconds, save the window as a PPM image and quit
                       (for development)
  --version            Print the version and quit
  --help               Print this help and quit";

struct App {
    ui: ParleySshUi,
    title: String,
    screenshot: Option<screenshot::Pending>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
            eframe::set_value(storage, SETTINGS_KEY, self.ui.settings());
        }
    }
}

enum Start {
    Run { screenshot: Option<String> },
    Version,
    Help,
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<Start, String> {
    let mut screenshot = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => return Ok(Start::Version),
            "--help" | "-h" => return Ok(Start::Help),
            "--screenshot" => screenshot = Some(args.next().ok_or("'--screenshot' needs a value")?),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(Start::Run { screenshot })
}

fn main() -> eframe::Result {
    let screenshot = match parse(std::env::args().skip(1)) {
        Ok(Start::Run { screenshot }) => screenshot,
        Ok(Start::Version) => {
            println!("parley-ssh {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Ok(Start::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(e) => {
            eprintln!("parley-ssh: {e}\n\n{USAGE}");
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
            match parley_app::logo() {
                Ok(logo) => ui.set_logo(logo),
                Err(e) => eprintln!("parley-ssh: could not load the menu bar icon: {e}"),
            }
            Ok(Box::new(App {
                ui,
                title: String::new(),
                screenshot: screenshot.map(screenshot::Pending::new),
            }))
        }),
    )
}
