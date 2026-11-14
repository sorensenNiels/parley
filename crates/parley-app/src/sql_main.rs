//! Parley SQL (R-SQL-45, D26): the SQL window on its own, without a 5250 session.
//!
//! Thin like `parley`: it opens the window and hands over the platform services from this
//! crate's library. The UI is `parley_ui_egui::ParleySqlUi`.

// Release builds on Windows are GUI apps: no console window opens next to Parley SQL.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a binary reports to the terminal"
)]

use eframe::egui;
use parley_app::{PARLEY_SQL_ID, SETTINGS_KEY, screenshot};
use parley_ui_egui::{ParleySqlUi, SqlConnection};

/// Where the saved connections are kept in eframe's storage (R-SQL-46).
const CONNECTIONS_KEY: &str = "connections";

const USAGE: &str = "\
Usage: parley-sql [OPTION] [CONNECTION]

Opens Parley SQL, the SQL window of Parley on its own, for SQL on IBM i. With CONNECTION,
the name of a saved connection, opens its sign-on straight away.

Options:
  --screenshot FILE    After a few seconds, save the window as a PPM image and quit
                       (for development)
  --version            Print the version and quit
  --help               Print this help and quit";

/// Runs the UI inside eframe.
struct App {
    ui: ParleySqlUi,
    title: String,
    connect_on_start: Option<usize>,
    screenshot: Option<screenshot::Pending>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(index) = self.connect_on_start.take() {
            self.ui.connect(index);
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
        // A screenshot run is for development: it must not replace the operator's settings.
        if self.screenshot.is_some() {
            return;
        }
        eframe::set_value(storage, CONNECTIONS_KEY, &self.ui.connections());
        eframe::set_value(storage, SETTINGS_KEY, self.ui.settings());
    }
}

/// What the command line asked for.
#[derive(Debug, Default)]
struct Args {
    connection: Option<String>,
    screenshot: Option<String>,
}

fn main() -> eframe::Result {
    let mut args = Args::default();
    let mut command_line = std::env::args().skip(1);
    while let Some(arg) = command_line.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("parley-sql {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--screenshot" => args.screenshot = command_line.next(),
            _ if !arg.starts_with('-') && args.connection.is_none() => {
                args.connection = Some(arg);
            }
            _ => {
                eprintln!("parley-sql: unknown option '{arg}'\n\n{USAGE}");
                std::process::exit(2);
            }
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Parley SQL")
        .with_app_id(PARLEY_SQL_ID)
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([640.0, 400.0]);
    match parley_app::window_icon() {
        Ok(icon) => viewport = viewport.with_icon(std::sync::Arc::new(icon)),
        Err(e) => eprintln!("parley-sql: could not load the window icon: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Parley SQL",
        options,
        Box::new(move |cc| {
            let connections: Vec<SqlConnection> = cc
                .storage
                .and_then(|s| eframe::get_value(s, CONNECTIONS_KEY))
                .unwrap_or_default();
            let mut ui = ParleySqlUi::new(connections);
            if let Some(settings) = cc.storage.and_then(|s| eframe::get_value(s, SETTINGS_KEY)) {
                ui.set_settings(settings);
            }
            ui.set_sql_helpers(parley_app::helpers::sql_helpers());
            ui.set_file_saver(Box::new(parley_app::helpers::SqlFileWindow));
            ui.set_api_keys(std::sync::Arc::new(parley_app::api_keys::Keychain));
            // The same snippet folder as Parley's SQL add-on (R-SQL-24).
            ui.set_snippet_store(Box::new(
                parley_app::snippets::SnippetFolders::beside_settings(),
            ));
            ui.set_update_checker(parley_app::update_checker());
            match parley_app::logo() {
                Ok(logo) => ui.set_logo(logo),
                Err(e) => eprintln!("parley-sql: could not load the menu bar icon: {e}"),
            }
            let connect_on_start = args.connection.as_deref().and_then(|name| {
                let found = ui.connection_index(name);
                if found.is_none() {
                    eprintln!("parley-sql: no saved connection called '{name}'");
                }
                found
            });
            Ok(Box::new(App {
                ui,
                title: String::new(),
                connect_on_start,
                screenshot: args.screenshot.map(screenshot::Pending::new),
            }))
        }),
    )
}
