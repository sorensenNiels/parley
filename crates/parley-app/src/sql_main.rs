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
use parley_app::{screenshot, sql_start};
use parley_ui_egui::{ParleySqlUi, SqlConnection};

/// Where the saved connections are kept in eframe's storage (R-SQL-46).
const CONNECTIONS_KEY: &str = "connections";
/// Where the settings are kept.
const SETTINGS_KEY: &str = "settings";

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

fn main() -> eframe::Result {
    let args = match sql_start::parse(std::env::args().skip(1)) {
        Ok(sql_start::Command::Run(args)) => args,
        Ok(sql_start::Command::Version) => {
            println!("parley-sql {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Ok(sql_start::Command::Help) => {
            println!("{}", sql_start::USAGE);
            return Ok(());
        }
        Err(e) => {
            eprintln!("parley-sql: {e}\n\n{}", sql_start::USAGE);
            std::process::exit(2);
        }
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Parley SQL")
        .with_app_id("parley-sql")
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
            // R-SQL-70: Parley's Tools ▸ SQL… names a host; a saved connection to it, or a new one.
            let connect_on_start = match args.start {
                Some(start) => Some(ui.connection_for(start)),
                None => args.connection.as_deref().and_then(|name| {
                    let found = ui.connection_index(name);
                    if found.is_none() {
                        eprintln!("parley-sql: no saved connection called '{name}'");
                    }
                    found
                }),
            };
            Ok(Box::new(App {
                ui,
                title: String::new(),
                connect_on_start,
                screenshot: args.screenshot.map(screenshot::Pending::new),
            }))
        }),
    )
}
