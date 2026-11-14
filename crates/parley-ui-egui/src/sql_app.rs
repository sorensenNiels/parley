//! Parley SQL (R-SQL-45, D26): the SQL window as a program of its own, for people who need SQL
//! on IBM i but no 5250 session. The same window as Parley's SQL add-on, drawn in the main
//! window under a header (File, Help, a gear for Settings) and over a status bar, with saved
//! connections (R-SQL-46) instead of saved sessions, and a settings dialog of its own pages
//! (R-SQL-47). Parley's SQL add-on is unchanged.

mod bars;
mod connections;

use egui::{CentralPanel, Margin, Panel, Ui, ViewportCommand};

pub use connections::SqlConnection;

use crate::addons::AddOn;
use crate::menu::{Logo, Program};
use crate::settings::{self, Settings};
use crate::sql::{self, SqlWindow};
use crate::{connect, theme, updates};

/// The start screen's words: saved connections.
const CONNECTIONS: connect::Words = connect::Words {
    none: "No connections yet.",
    new: "New connection…",
};

/// Parley SQL's whole UI. The launcher keeps [`ParleySqlUi::connections`] and
/// [`ParleySqlUi::settings`].
#[derive(Debug)]
pub struct ParleySqlUi {
    connections: Vec<SqlConnection>,
    /// The connection the window is open on, as it was when it opened.
    current: Option<SqlConnection>,
    window: Option<SqlWindow>,
    settings: Settings,
    applied: Option<Settings>,
    settings_dialog: Option<settings::Dialog>,
    editor: Option<connections::Editor>,
    logo: Logo,
    updates: updates::Updates,
    helpers: Option<std::path::PathBuf>,
    files: Option<Box<dyn sql::SqlFiles>>,
    snippets: Option<Box<dyn sql::snippets::SnippetStore>>,
    /// Why the window could not open, in a dialog (R-SQL-12).
    notice: Option<String>,
    /// Said in the status bar while no window is open: why the last one closed.
    note: Option<String>,
    scripts_kept: u64,
    /// The main window was closed while changes were not committed: it closes once the SQL
    /// window has asked and closed (R-SQL-5).
    exit_after_window: bool,
}

impl ParleySqlUi {
    pub fn new(connections: Vec<SqlConnection>) -> Self {
        Self {
            connections,
            current: None,
            window: None,
            settings: Settings::default(),
            applied: None,
            settings_dialog: None,
            editor: None,
            logo: Logo::default(),
            updates: updates::Updates::default(),
            helpers: None,
            files: None,
            snippets: None,
            notice: None,
            note: None,
            scripts_kept: 0,
            exit_after_window: false,
        }
    }

    pub fn connections(&self) -> &[SqlConnection] {
        &self.connections
    }

    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    pub fn set_logo(&mut self, image: egui::ColorImage) {
        self.logo = Logo::new(image);
    }

    /// The folder of the SQL helper's jars; `None` when this installation has none (R-SQL-13).
    pub fn set_sql_helpers(&mut self, folder: Option<std::path::PathBuf>) {
        self.helpers = folder;
    }

    pub fn set_file_saver(&mut self, saver: Box<dyn sql::SqlFiles>) {
        self.files = Some(saver);
    }

    pub fn set_snippet_store(&mut self, store: Box<dyn sql::snippets::SnippetStore>) {
        self.snippets = Some(store);
    }

    pub fn set_update_checker(&mut self, checker: updates::Checker) {
        self.updates.set_checker(checker);
    }

    /// The saved connection called `name`, if any (the command line).
    pub fn connection_index(&self, name: &str) -> Option<usize> {
        self.connections
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// The title bar: the connection in use, then the program.
    pub fn title(&self) -> String {
        let name = Program::ParleySql.name();
        match &self.current {
            Some(connection) => format!("{} – {name}", connection.name),
            None => name.to_owned(),
        }
    }

    /// Opens the SQL window on the saved connection at `index`, at its sign-on; says why
    /// when it cannot (R-SQL-12).
    pub fn connect(&mut self, index: usize) {
        if self.window.is_some() {
            return;
        }
        let Some(connection) = self.connections.get(index).cloned() else {
            return;
        };
        match sql::hosting::open(
            &connection.name,
            &connection.config(),
            self.helpers.as_deref(),
            &connection.user,
            &self.settings,
        ) {
            Ok(window) => {
                self.window = Some(window.embedded());
                self.current = Some(connection);
                self.note = None;
            }
            Err(e) => self.notice = Some(e),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        if self.applied.is_none() && self.settings.check_for_updates {
            self.updates.start(ui.ctx(), updates::Check::AtStart);
        }
        self.updates.collect();
        if self.applied.as_ref() != Some(&self.settings) {
            // Parley SQL has no terminal, so a terminal font that could not load is no news.
            let _ = settings::apply(ui.ctx(), self.applied.as_ref(), &self.settings);
            self.applied = Some(self.settings.clone());
        }
        self.guard_exit(ui.ctx());

        let menu_frame = theme::bar(ui, Margin::symmetric(8, 3));
        let status_frame = theme::bar(ui, Margin::symmetric(8, 5));
        let action = self.menu_panel(ui, menu_frame);
        self.status_panel(ui, status_frame);
        if self.window.is_some() {
            self.show_window(ui);
        } else {
            self.start_screen(ui);
        }
        if let Some(action) = action {
            self.carry_out(ui.ctx(), action);
        }
        self.show_editor(ui.ctx());
        self.show_settings(ui.ctx());
        sql::hosting::notice_dialog(ui.ctx(), &mut self.notice);
    }

    /// Closing the main window with changes not committed asks first, as the SQL window
    /// does (R-SQL-5); it closes once that is answered.
    fn guard_exit(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if let Some(window) = &mut self.window
            && window.status().uncommitted
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            window.request_close();
            self.exit_after_window = true;
        }
    }

    fn menu_panel(&mut self, ui: &mut Ui, frame: egui::Frame) -> Option<bars::Action> {
        let logo = self.logo.texture(ui.ctx()).cloned();
        let phase = self.window.as_ref().map(|w| w.status().phase);
        let header = bars::Header {
            logo: logo.as_ref(),
            connections: &self.connections,
            current: self.current.as_ref().zip(phase),
            newer: self.updates.available(),
        };
        Panel::top("sql-app-menu")
            .frame(frame)
            .show(ui, |ui| bars::menu_bar(ui, &header))
            .inner
    }

    fn status_panel(&self, ui: &mut Ui, frame: egui::Frame) {
        let window = self
            .current
            .as_ref()
            .zip(self.window.as_ref().map(SqlWindow::status));
        let footer = bars::Footer {
            window,
            note: self.note.as_deref(),
        };
        Panel::bottom("sql-app-status")
            .frame(frame)
            .show(ui, |ui| bars::status_bar(ui, &footer));
    }

    fn start_screen(&mut self, ui: &mut Ui) {
        CentralPanel::default().show(ui, |ui| {
            let entries: Vec<_> = self.connections.iter().map(SqlConnection::entry).collect();
            match connect::show(ui, CONNECTIONS, &entries, None) {
                Some(connect::Choice::Connect(i)) => self.connect(i),
                Some(connect::Choice::Edit(i)) => self.open_editor(Some(i)),
                Some(connect::Choice::New) => self.open_editor(None),
                None => {}
            }
        });
    }

    /// The SQL window in the rest of the main window; what it changed into the settings.
    fn show_window(&mut self, ui: &mut Ui) {
        let assistant = self.settings.addons.enabled(AddOn::Assistant);
        let Some((window, current)) = self.window.as_mut().zip(self.current.as_ref()) else {
            return;
        };
        let name = current.name.clone();
        let history = self
            .settings
            .sql_history
            .get(&name)
            .cloned()
            .unwrap_or_default();
        let around = sql::Around {
            history: &history,
            files: self.files.as_deref(),
            snippets: self.snippets.as_deref(),
            shared: &self.settings.sql_snippets_shared,
            // One connection at a time: no Run on… (D26).
            systems: None,
        };
        window.set_assistant(assistant);
        let open = window.show_in(ui, around);
        sql::hosting::keep(window, &name, &mut self.settings, &mut self.scripts_kept);
        self.remember_user(&name);
        if !open {
            self.window = None;
            self.current = None;
            self.note = Some(format!("Disconnected from {name}."));
            if std::mem::take(&mut self.exit_after_window) {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    /// Once signed on, the user goes into the saved connection for next time (R-SQL-46).
    fn remember_user(&mut self, name: &str) {
        let Some(window) = &self.window else {
            return;
        };
        let Some(connected) = window.status().connected else {
            return;
        };
        let user = window.user.trim();
        let user = if user.is_empty() {
            connected.user.as_str()
        } else {
            user
        };
        if let Some(saved) = self.connections.iter_mut().find(|c| c.name == name)
            && saved.user != user
        {
            user.clone_into(&mut saved.user);
        }
    }

    fn carry_out(&mut self, ctx: &egui::Context, action: bars::Action) {
        match action {
            bars::Action::NewConnection => self.open_editor(None),
            bars::Action::EditConnections => {
                let index = self
                    .current
                    .as_ref()
                    .and_then(|c| self.connection_index(&c.name))
                    .or((!self.connections.is_empty()).then_some(0));
                self.open_editor(index);
            }
            bars::Action::Connect(i) => self.connect(i),
            bars::Action::Disconnect => {
                if let Some(window) = &mut self.window {
                    window.request_close();
                }
            }
            bars::Action::Exit => ctx.send_viewport_cmd(ViewportCommand::Close),
            bars::Action::Settings => self.open_settings(ctx, false),
            bars::Action::CheckForUpdates => {
                self.open_settings(ctx, true);
                self.updates.start(ctx, updates::Check::Now);
            }
        }
    }

    fn open_editor(&mut self, index: Option<usize>) {
        self.editor = Some(connections::Editor::open(&self.connections, index));
    }

    fn show_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let outcome = editor.show(ctx, &mut self.connections, self.window.is_some());
        if let Some((old, new)) = editor.take_renamed() {
            sql::hosting::rename(&mut self.settings, &old, &new);
        }
        match outcome {
            Some(connections::Outcome::Close) => self.editor = None,
            Some(connections::Outcome::Connect(index)) => {
                self.editor = None;
                self.connect(index);
            }
            None => {}
        }
    }

    fn open_settings(&mut self, ctx: &egui::Context, about: bool) {
        if self.settings_dialog.is_some() {
            return;
        }
        let dialog = settings::Dialog::open(ctx, &self.settings)
            .for_program(Program::ParleySql)
            .with_sql_helpers(self.helpers.clone());
        self.settings_dialog = Some(if about {
            dialog.on_about_page()
        } else {
            dialog
        });
    }

    fn show_settings(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.settings_dialog else {
            return;
        };
        let closed = dialog.show(ctx, &mut self.settings, self.updates.status());
        if dialog.take_check_request() {
            self.updates.start(ctx, updates::Check::Now);
        }
        match closed {
            Some(settings::Closed::Keep) => self.settings_dialog = None,
            Some(settings::Closed::Revert) => {
                self.settings = dialog.original().clone();
                self.settings_dialog = None;
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    use super::*;

    const SETTLE_STEPS: u64 = 16;

    fn prod() -> SqlConnection {
        SqlConnection {
            name: "Prod".to_owned(),
            host: "prod.example.com".to_owned(),
            ..SqlConnection::default()
        }
    }

    fn app_with(connections: Vec<SqlConnection>) -> Harness<'static, ParleySqlUi> {
        Harness::builder()
            .with_size(egui::vec2(1000.0, 700.0))
            .with_max_steps(SETTLE_STEPS)
            .build_ui_state(
                |ui, app: &mut ParleySqlUi| app.ui(ui),
                ParleySqlUi::new(connections),
            )
    }

    #[test]
    fn the_header_has_the_name_file_help_and_a_gear_at_the_right_end_r_sql_45() {
        let harness = app_with(Vec::new());
        harness.get_by_label("Parley SQL");
        let file = harness.get_by_label("File").rect();
        let help = harness.get_by_label("Help").rect();
        let gear = harness.get_by_label("Settings").rect();
        assert!(file.left() < help.left(), "File, then Help");
        assert!(
            gear.left() > 900.0,
            "the gear is at the right end: {gear:?}"
        );
        assert!(
            (gear.center().y - file.center().y).abs() < 2.0,
            "in the header"
        );
        assert!(
            harness.query_by_label("Tools").is_none(),
            "no terminal menus"
        );
    }

    #[test]
    fn the_status_bar_says_offline_until_a_connection_opens_r_sql_45() {
        let harness = app_with(vec![prod()]);
        harness.get_by_label("Offline");
        assert_eq!(harness.state().title(), "Parley SQL");
    }

    #[test]
    fn the_gear_opens_settings_with_the_sql_pages_only_r_sql_47() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("Settings").click();
        harness.run();
        for page in ["Appearance", "SQL", "About"] {
            harness.get_by_label(page);
        }
        for page in ["Terminal", "Keyboard", "Add-ons"] {
            assert!(harness.query_by_label(page).is_none(), "{page} is Parley's");
        }
        harness.get_by_label("SQL").click();
        harness.run();
        harness.get_by_label("SQL assistant");
        harness.get_by_label("Rows at a time");
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before, "D12");
    }

    #[test]
    fn a_new_connection_is_saved_from_the_start_screen_r_sql_46() {
        let mut harness = app_with(Vec::new());
        harness.get_by_label("No connections yet.");
        harness.get_by_label("New connection…").click();
        harness.run();
        harness.get_by_label("Name").focus();
        harness.run();
        harness.get_by_label("Name").type_text("Test");
        harness.run();
        harness.get_by_label("Host").focus();
        harness.run();
        harness.get_by_label("Host").type_text("test.example.com");
        harness.run();
        harness.get_by_label("Save").click();
        harness.run();
        let saved = harness.state().connections();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "Test");
        assert!(saved[0].tls, "TLS by default");
    }

    #[test]
    fn the_connection_dialog_keeps_its_place_from_frame_to_frame_r_sql_46() {
        // D12: a dialog that grows over frames moves its buttons away from the pointer.
        let mut harness = app_with(vec![prod()]);
        harness.get_by_label("Edit…").click();
        harness.run();
        harness.get_by_label("Edit connection");
        let before = harness.get_by_label("Save").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Save").rect(), before);
    }

    /// The app with the SQL window open on Prod at its sign-on, as if Connect had found Java.
    fn signing_on() -> Harness<'static, ParleySqlUi> {
        with_window(vec![prod()], prod_window())
    }

    /// The SQL window on Prod, embedded, as Connect opens it.
    fn prod_window() -> SqlWindow {
        SqlWindow::open(
            "Prod",
            &prod().config(),
            std::path::PathBuf::from("java"),
            parley_core::sql::helper::HelperFiles::in_folder(std::path::Path::new("/none")),
            "",
            &[],
        )
        .embedded()
    }

    /// The app with these connections and `window` open on Prod.
    fn with_window(
        connections: Vec<SqlConnection>,
        window: SqlWindow,
    ) -> Harness<'static, ParleySqlUi> {
        let mut harness = app_with(connections);
        harness.state_mut().window = Some(window);
        harness.state_mut().current = Some(prod());
        harness.run();
        harness
    }

    #[test]
    fn the_sql_window_fills_the_main_window_under_the_header_r_sql_45() {
        let mut harness = signing_on();
        harness.get_by_label("Sign on to prod.example.com");
        harness.get_by_label("Not signed on");
        assert_eq!(harness.state().title(), "Prod – Parley SQL");
        harness.get_by_label("Parley SQL");
        harness.get_by_label("Settings");
        harness.get_by_label("Disconnect").click();
        harness.run();
        assert!(harness.state().window.is_none(), "back to the start screen");
        harness.get_by_label("Disconnected from Prod.");
        harness.get_by_label("New connection…");
    }

    #[test]
    fn parley_sql_runs_on_one_connection_only_d26() {
        use egui_kittest::kittest::Queryable;
        let test = SqlConnection {
            name: "Test".to_owned(),
            host: "test.example.com".to_owned(),
            ..SqlConnection::default()
        };
        let harness = with_window(vec![prod(), test], prod_window().signed_on_for_tests());
        harness.get_by_label("Run all");
        assert!(
            harness.query_by_label("Run on…").is_none(),
            "no Run on… although Test is saved"
        );
    }

    #[test]
    fn connecting_without_the_helper_says_why_the_window_cannot_open_r_sql_45() {
        let mut harness = app_with(vec![prod()]);
        harness.get_by_label("Connect").click();
        harness.run();
        harness.get_by_label_contains("installed without the SQL helper");
        assert!(harness.state().window.is_none());
        assert_eq!(harness.state().title(), "Parley SQL");
    }
}
