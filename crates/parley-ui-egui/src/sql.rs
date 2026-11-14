//! The SQL window (R-SQL-6..9, D20): a window of its own beside the main one, where the operator
//! signs on to the session's host, writes SQL, runs it through the SQL helper and reads the
//! results. The helper runs on a thread of its own (`parley_core::sql::runner`), so the window
//! never waits for the host.

pub(crate) mod assistant;
mod browser;
mod chart;
mod database;
pub mod editor;
pub mod grid;
pub mod hosting;
pub mod keys;
pub mod layout;
mod parameters;
pub mod results;
pub mod snippets;
mod tools;

use std::collections::VecDeque;
use std::path::PathBuf;

use egui::{CentralPanel, Key, Panel, RichText, ScrollArea, TextEdit, Ui};
use parley_core::SessionConfig;
use parley_core::password::Password;
use parley_core::sql::helper::HelperFiles;
use parley_core::sql::params;
use parley_core::sql::protocol::{
    self, Connect, Connected, Done, Hello, MoreRows, Outcome, Request,
};
use parley_core::sql::runner::{Event, Runner};
use parley_core::sql::text::{self, Risk};

use editor::gutter;
use grid::ResultView;

use crate::{dialog, icons, theme};

/// Files the SQL window reads and writes through the system's file window, in the app
/// (R-SQL-8, R-SQL-21). `Ok(None)` is the operator cancelling.
pub trait SqlFiles: std::fmt::Debug {
    /// Saves an export where the operator chooses; the path saved to.
    fn export_csv(&self, suggested: &str, bytes: &[u8]) -> Result<Option<String>, String>;
    /// A `.sql` file the operator picks: its path and text.
    fn open_script(&self) -> Result<Option<(String, String)>, String>;
    /// Saves `text` to `path`, or where the operator chooses when `path` is `None`; the path.
    fn save_script(
        &self,
        path: Option<&str>,
        suggested: &str,
        text: &str,
    ) -> Result<Option<String>, String>;
}

/// What the SQL window is given by Parley each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Around<'a> {
    /// The statements run for this session, oldest first (R-SQL-7).
    pub history: &'a [String],
    /// The file window (R-SQL-8, R-SQL-21); without it, no Export or Open….
    pub files: Option<&'a dyn SqlFiles>,
    /// The snippet folders (R-SQL-24); without them, only the starters.
    pub snippets: Option<&'a dyn snippets::SnippetStore>,
    /// The shared snippet folder; empty for none (R-SQL-24).
    pub shared: &'a str,
    /// Where the results go: below the editor, beside it, or beside it when there is room
    /// (R-SQL-49).
    pub results: layout::ResultsPlace,
    /// Brackets and quotes close themselves in the editor (R-SQL-55).
    pub close_pairs: bool,
    /// The window's shortcuts (R-SQL-60); `None` for the default keys, as in Parley.
    pub keys: Option<&'a keys::SqlKeys>,
}

/// How far the window is, for Parley SQL's status bar (R-SQL-45).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    SignIn,
    Connecting,
    /// Signed on and waiting for nothing.
    Ready,
    /// Signed on, with a request in flight.
    Busy,
}

/// A small icon button that folds or opens the side panel, named `name`; true when clicked.
fn panel_button(ui: &mut Ui, icon: &str, name: &str) -> bool {
    let glyph = icons::icon(icon).color(theme::palette(ui).muted);
    icons::add_named(ui, true, theme::ghost_button(glyph), name)
        .on_hover_text(name)
        .clicked()
}

/// What the window says about itself, for the status bar of Parley SQL (R-SQL-45).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowStatus {
    pub phase: Phase,
    /// The user, job and release once signed on.
    pub connected: Option<Connected>,
    pub autocommit: bool,
    pub uncommitted: bool,
    /// The last error, if the window shows one.
    pub error: Option<String>,
}

/// Beside the results, the height the editor leaves below its text: the tab strip, the Run
/// buttons and a line of error.
const BELOW_THE_TEXT: f32 = 110.0;

/// The id salt of the editor's part of the window, the same below the results and beside them,
/// so the editor keeps its focus, cursor and scrolling when the arrangement changes (R-SQL-49).
const WRITING: &str = "sql-writing";

/// Where the window is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// A window of its own beside Parley's (R-SQL-6).
    OwnWindow,
    /// Parley SQL's main window (R-SQL-45).
    Embedded,
}

/// What the panel beside the editor shows (R-SQL-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    History,
    Snippets,
    /// The schema browser (R-SQL-34).
    Database,
}

/// Whether the panel with History, Snippets and Database is shown (R-SQL-51).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidePanel {
    Open,
    /// Folded to a narrow strip with a button that opens it again.
    Folded,
}

/// How far the window is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    SignIn,
    Connecting,
    Ready,
}

/// What the request in flight was.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Waiting {
    Hello,
    Connect,
    Run(String),
    /// More rows from this cursor.
    Fetch(u64),
    Autocommit(bool),
    Commit,
    Rollback,
    /// The help for the error at `index` in Messages, message `id`: from the job log, or
    /// from its message file (R-SQL-31).
    Explain {
        index: usize,
        id: String,
        file: bool,
    },
    JobLog,
    /// The current schema and library list (R-SQL-32).
    Job,
}

/// Results or messages under the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Results,
    Messages,
}

/// The answer to the question asked on closing with uncommitted changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closing {
    Open,
    /// Asking Commit, Roll back or Cancel.
    Asking,
    /// Waiting for a commit or rollback before closing.
    AfterCommit,
    Closed,
}

/// The SQL window for one session.
#[derive(Debug)]
pub struct SqlWindow {
    config: SessionConfig,
    java: PathBuf,
    files: HelperFiles,
    stage: Stage,
    pub user: String,
    password: String,
    runner: Option<Runner>,
    connected: Option<Connected>,
    /// The scripts in the editor (R-SQL-20).
    scripts: editor::Scripts,
    /// Statements still to run (Run all).
    queue: VecDeque<String>,
    waiting: Option<(u64, Waiting)>,
    /// When the statement running was sent, for its outcome's time (R-SQL-62).
    run_started: Option<std::time::Instant>,
    results: results::Results,
    tab: Tab,
    side: Side,
    snippets: snippets::SnippetPanel,
    browser: browser::Browser,
    messages: Vec<(bool, String)>,
    /// The help asked for an error in Messages (R-SQL-31).
    helps: tools::Helps,
    /// The SQL job's current schema and library list, once read (R-SQL-32).
    job: Option<tools::JobContext>,
    /// Read the job again when idle: after Connect and after a statement that may change it.
    job_stale: bool,
    /// The library typed in the schema menu.
    add_library: String,
    error_line: Option<String>,
    autocommit: bool,
    uncommitted: bool,
    confirm: Option<(String, Risk)>,
    closing: Closing,
    /// Statements run since the last [`SqlWindow::take_ran`], for the history.
    ran: Vec<String>,
    /// The sign-on, sent once the helper's hello is read; it holds the password until then.
    pending_connect: Option<Request>,
    /// The form for a statement's parameter markers while it is open (R-SQL-40).
    parameters: Option<parameters::ParamForm>,
    /// The values given for each statement's markers, by its text, for this window.
    param_values: std::collections::BTreeMap<String, Vec<parameters::Value>>,
    /// The second connection, for the catalog (R-SQL-33, D21).
    database: database::Database,
    /// The SQL assistant (R-SQL-44), shown while its add-on is on (R-ADD-3).
    assistant: assistant::Assistant,
    /// The same sign-on for it, held until the first has signed on, then sent or dropped.
    pending_catalog: Option<Request>,
    /// What the result columns' headings say (R-SQL-29); kept in the settings.
    pub header: grid::HeaderMode,
    /// Rows fetched at a time, and seconds before a statement is stopped (R-SQL-18).
    rows: u32,
    timeout: u32,
    place: Place,
    /// Results beside the editor: the editor's share of the width (R-SQL-49).
    editor_share: f32,
    /// Results beside the editor: the height of what is above and below its text, measured
    /// each frame so that the text fills the rest of the column (R-SQL-49).
    around_the_text: f32,
    /// Results below the editor: the height its text was dragged to (R-SQL-50).
    editor_height: Option<f32>,
    /// Whether History, Snippets and Database are shown or folded away (R-SQL-51).
    side_panel: SidePanel,
}

impl SqlWindow {
    /// The window on `config`'s host, run with `java` and the helper in `files`; `user` is the
    /// one signed on with last.
    pub fn open(
        config: &SessionConfig,
        java: PathBuf,
        files: HelperFiles,
        user: &str,
        scripts: &[editor::SavedScript],
    ) -> Self {
        Self {
            config: config.clone(),
            java,
            files,
            stage: Stage::SignIn,
            user: user.to_owned(),
            password: String::new(),
            runner: None,
            connected: None,
            scripts: editor::Scripts::restore(scripts),
            queue: VecDeque::new(),
            waiting: None,
            run_started: None,
            results: results::Results::reading(config.code_page),
            tab: Tab::Results,
            side: Side::History,
            snippets: snippets::SnippetPanel::default(),
            browser: browser::Browser::default(),
            messages: Vec::new(),
            helps: tools::Helps::new(),
            job: None,
            job_stale: false,
            add_library: String::new(),
            error_line: None,
            autocommit: true,
            uncommitted: false,
            confirm: None,
            closing: Closing::Open,
            ran: Vec::new(),
            pending_connect: None,
            database: database::Database::default(),
            pending_catalog: None,
            parameters: None,
            assistant: assistant::Assistant::default(),
            param_values: std::collections::BTreeMap::new(),
            header: grid::HeaderMode::default(),
            rows: protocol::FIRST_ROWS,
            timeout: 0,
            place: Place::OwnWindow,
            editor_share: layout::DEFAULT_SHARE,
            around_the_text: BELOW_THE_TEXT,
            editor_height: None,
            side_panel: SidePanel::Open,
        }
    }

    /// Drawn with [`SqlWindow::show_in`] in Parley SQL's main window (R-SQL-45): its Close
    /// button says Disconnect, and the main window asks before closing (R-SQL-5).
    pub const fn embedded(mut self) -> Self {
        self.place = Place::Embedded;
        self
    }

    /// The window as if signed on, for the tests of the programs that hold it.
    #[cfg(test)]
    pub(crate) fn signed_on_for_tests(mut self) -> Self {
        self.stage = Stage::Ready;
        self
    }

    /// Where the window is, for a status bar (R-SQL-45).
    pub fn status(&self) -> WindowStatus {
        let phase = match self.stage {
            Stage::SignIn => Phase::SignIn,
            Stage::Connecting => Phase::Connecting,
            Stage::Ready if self.waiting.is_some() => Phase::Busy,
            Stage::Ready => Phase::Ready,
        };
        WindowStatus {
            phase,
            connected: self.connected.clone(),
            autocommit: self.autocommit,
            uncommitted: self.uncommitted,
            error: self.error_line.clone(),
        }
    }

    /// Starts with the column headings saying `header` (R-SQL-29).
    pub const fn with_header(mut self, header: grid::HeaderMode) -> Self {
        self.header = header;
        self
    }

    /// The libraries pinned in the schema browser for this session (R-SQL-34).
    pub fn with_pinned(mut self, pinned: Vec<String>) -> Self {
        self.browser = browser::Browser::with_pinned(pinned);
        self
    }

    /// Shows the SQL assistant while it is on (R-ADD-3), asking `config`'s provider with a key
    /// from the environment or `keys` (R-SQL-48).
    pub fn set_assistant(
        &mut self,
        on: bool,
        config: &parley_core::sql::assistant::AssistantConfig,
        keys: Option<&std::sync::Arc<dyn assistant::ApiKeys>>,
    ) {
        self.assistant.on = on;
        if self.assistant.config != *config {
            self.assistant.config.clone_from(config);
        }
        self.assistant.keys = keys.cloned();
    }

    /// The libraries pinned now, to keep in the settings.
    pub fn pinned(&self) -> &[String] {
        self.browser.pinned()
    }

    /// Fetches `rows` at a time and stops a statement after `timeout` seconds (0: never).
    pub const fn with_limits(mut self, rows: u32, timeout: u32) -> Self {
        self.rows = rows;
        self.timeout = timeout;
        self
    }

    /// The scripts as the settings keep them, and a count that changes with them (R-SQL-20).
    pub fn scripts(&self) -> (u64, Vec<editor::SavedScript>) {
        (self.scripts.generation(), self.scripts.to_saved())
    }

    /// Changes to the scripts so far: when it moves on, [`SqlWindow::scripts`] is worth keeping.
    pub const fn scripts_generation(&self) -> u64 {
        self.scripts.generation()
    }

    /// The statements run since last asked, for the history (R-SQL-7).
    pub fn take_ran(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ran)
    }

    /// Closes the window, asking first if changes are not committed (R-SQL-5); the session
    /// disconnected, or the operator closed it.
    pub fn request_close(&mut self) {
        if self.closing == Closing::Open {
            self.closing = if self.uncommitted {
                Closing::Asking
            } else {
                Closing::Closed
            };
        }
    }

    /// Draws the window in `ui`, Parley SQL's main window (R-SQL-45); false once it is closed.
    pub fn show_in(&mut self, ui: &mut Ui, around: Around<'_>) -> bool {
        self.step(|window| window.ui(ui, around))
    }

    /// Reads the helpers, then draws with `draw` unless closed; false once it is closed.
    fn step(&mut self, draw: impl FnOnce(&mut Self)) -> bool {
        self.read_events();
        if self.closing != Closing::Closed {
            draw(self);
        }
        if self.closing == Closing::Closed {
            self.finish();
            return false;
        }
        true
    }

    fn ui(&mut self, ui: &mut Ui, around: Around<'_>) {
        let top = theme::bar(ui, egui::Margin::symmetric(12, 8));
        Panel::top("sql-top")
            .frame(top)
            .show(ui, |ui| self.top_bar(ui));
        match self.stage {
            Stage::SignIn | Stage::Connecting => {
                CentralPanel::default().show(ui, |ui| self.sign_in(ui));
            }
            Stage::Ready => {
                match self.side_panel {
                    SidePanel::Open => {
                        Panel::right("sql-history")
                            .resizable(true)
                            .default_size(250.0)
                            .show(ui, |ui| self.side(ui, around));
                    }
                    SidePanel::Folded => {
                        Panel::right("sql-history-folded")
                            .resizable(false)
                            .exact_size(36.0)
                            .show(ui, |ui| self.folded_side(ui));
                    }
                }
                CentralPanel::default().show(ui, |ui| self.work(ui, around));
            }
        }
        if self.scripts.take_run_asked() {
            self.run_current();
        }
        if let Some(i) = self.scripts.take_snippet_chosen()
            && let Some(used) = self.snippets.start(i, false)
        {
            self.use_snippet(used);
        }
        let used = self
            .snippets
            .dialogs(ui.ctx(), around.snippets, around.shared);
        if let Some(used) = used {
            self.use_snippet(used);
        }
        self.confirm_dialog(ui.ctx());
        self.parameters_dialog(ui.ctx());
        self.closing_dialog(ui.ctx());
    }

    // ---------------------------------------------------------------- the top bar

    fn top_bar(&mut self, ui: &mut Ui) {
        let p = theme::palette(ui);
        ui.horizontal(|ui| {
            ui.label(icons::icon(icons::DATABASE).color(p.muted));
            match &self.connected {
                Some(c) => {
                    ui.label(theme::semibold(&c.user));
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · job {}",
                            self.config.host, c.release, c.job
                        ))
                        .color(p.muted),
                    );
                    self.schema_menu(ui);
                    if let Some((text, problem)) = self.database.status() {
                        if problem {
                            ui.label(icons::icon(icons::INFO).color(p.warning_text))
                                .on_hover_text(text);
                        } else {
                            ui.label(icons::icon(icons::DATABASE).color(p.muted).small())
                                .on_hover_text(text);
                        }
                    }
                }
                None => {
                    ui.label(RichText::new(&self.config.host).color(p.muted));
                }
            }
            theme::button_row(ui, |ui| {
                let close = match self.place {
                    Place::OwnWindow => "Close",
                    Place::Embedded => "Disconnect",
                };
                if ui.button(close).clicked() {
                    self.request_close();
                }
                if self.stage == Stage::Ready {
                    self.commit_controls(ui);
                }
            });
        });
        if !self.config.tls {
            theme::warning(
                ui,
                "Not encrypted: this host's connection has no TLS, so statements and results travel as plain text.",
            );
        }
    }

    /// Autocommit, and Commit and Rollback while it is off (R-SQL-5).
    fn commit_controls(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none();
        if !self.autocommit {
            let commit = ui.add_enabled(idle && self.uncommitted, egui::Button::new("Commit"));
            if commit.clicked() {
                self.send(Request::Commit, Waiting::Commit);
            }
            if ui
                .add_enabled(idle && self.uncommitted, egui::Button::new("Rollback"))
                .clicked()
            {
                self.send(Request::Rollback, Waiting::Rollback);
            }
        }
        let mut on = self.autocommit;
        let switch = ui
            .add_enabled_ui(idle && !self.uncommitted, |ui| theme::switch(ui, &mut on))
            .inner;
        let label = ui.label("Autocommit");
        switch
            .labelled_by(label.id)
            .on_hover_text("Off: changes wait for Commit or Rollback");
        if on != self.autocommit {
            self.send(Request::Autocommit { on }, Waiting::Autocommit(on));
        }
    }

    // ---------------------------------------------------------------- signing in

    fn sign_in(&mut self, ui: &mut Ui) {
        let connecting = self.stage == Stage::Connecting;
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(380.0);
            theme::card(ui).show(ui, |ui| {
                ui.label(theme::semibold(format!("Sign on to {}", self.config.host)));
                theme::hint(
                    ui,
                    "Parley keeps no password: it is asked each time, and used once.",
                );
                ui.add_space(8.0);
                egui::Grid::new("sql-sign-in")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        let user = ui.label("User");
                        theme::sized_edit(ui, 220.0, TextEdit::singleline(&mut self.user))
                            .labelled_by(user.id);
                        ui.end_row();
                        let password = ui.label("Password");
                        let field = theme::sized_edit(
                            ui,
                            220.0,
                            TextEdit::singleline(&mut self.password).password(true),
                        )
                        .labelled_by(password.id);
                        ui.end_row();
                        if field.lost_focus()
                            && ui.input(|i| i.key_pressed(Key::Enter))
                            && !connecting
                        {
                            self.connect(ui.ctx());
                        }
                    });
                if let Some(error) = &self.error_line {
                    theme::error(ui, error);
                }
                ui.add_space(8.0);
                theme::button_row(ui, |ui| {
                    let ready =
                        !connecting && !self.user.trim().is_empty() && !self.password.is_empty();
                    let label = if connecting {
                        "Connecting…"
                    } else {
                        "Connect"
                    };
                    if ui
                        .add_enabled(ready, theme::primary_button(ui, label))
                        .clicked()
                    {
                        self.connect(ui.ctx());
                    }
                });
            });
        });
    }

    /// Starts the helper and signs on once; the password field is emptied at once.
    fn connect(&mut self, ctx: &egui::Context) {
        // Taken out of the field first: whatever happens next, it is not kept there.
        let connect = Request::Connect(Connect {
            host: self.config.host.clone(),
            tls: self.config.tls,
            user: self.user.trim().to_owned(),
            password: Password::new(std::mem::take(&mut self.password)),
            ca_file: self.config.ca_file.clone(),
        });
        let wake = ctx.clone();
        match Runner::start(&self.java, &self.files, move || wake.request_repaint()) {
            Ok(runner) => self.runner = Some(runner),
            Err(e) => {
                self.error_line = Some(e.to_string());
                return;
            }
        }
        self.error_line = None;
        self.stage = Stage::Connecting;
        self.send(Request::Hello, Waiting::Hello);
        // R-SQL-33: the second helper starts now, and signs on after the first has.
        self.database.start(&self.java, &self.files, ctx);
        self.pending_catalog = Some(connect.clone());
        // Sent once the hello shows the helper speaks this protocol.
        self.pending_connect = Some(connect);
    }

    // ---------------------------------------------------------------- history and snippets

    /// History or Snippets, beside the editor (R-SQL-7, R-SQL-25).
    fn side(&mut self, ui: &mut Ui, around: Around<'_>) {
        ui.horizontal(|ui| {
            theme::segmented(
                ui,
                &mut self.side,
                &[
                    (Side::History, "History"),
                    (Side::Snippets, "Snippets"),
                    (Side::Database, "Database"),
                ],
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if panel_button(ui, icons::CARET_RIGHT, "Hide the side panel") {
                    self.side_panel = SidePanel::Folded;
                }
            });
        });
        ui.add_space(6.0);
        match self.side {
            Side::History => self.history(ui, around.history),
            Side::Snippets => {
                if let Some(used) = self.snippets.show(ui, around.snippets, around.shared) {
                    self.use_snippet(used);
                }
            }
            Side::Database => {
                let libraries = self
                    .job
                    .as_ref()
                    .map(|j| j.libraries.clone())
                    .unwrap_or_default();
                if let Some(pick) = self.browser.show(ui, &libraries, &mut self.database) {
                    self.use_pick(pick);
                }
            }
        }
    }

    /// The Ask row and the question's progress; its SQL opens in a new tab (R-SQL-44).
    fn assistant_row(&mut self, ui: &mut Ui) {
        if self.assistant.row(ui) {
            let release = self
                .connected
                .as_ref()
                .map_or("", |c| c.release.as_str())
                .to_owned();
            let (schema, libraries) = self.job.as_ref().map_or_else(Default::default, |j| {
                (
                    j.schema.clone(),
                    j.libraries
                        .iter()
                        .map(|(l, _)| l.clone())
                        .collect::<Vec<_>>(),
                )
            });
            self.assistant.ask(ui.ctx(), &release, &schema, &libraries);
        }
        if let Some(answer) = self
            .assistant
            .step(ui.ctx(), &mut self.browser, &mut self.database)
        {
            self.scripts
                .open_new("Assistant", &assistant::script(&answer));
        }
        ui.add_space(4.0);
    }

    /// Offers names for what is being typed, from the schema browser (R-SQL-36).
    fn complete(&mut self) {
        let Some((typed, statement)) = self.scripts.completing() else {
            return;
        };
        let (schema, libraries) = self
            .job
            .as_ref()
            .map(|j| (Some(j.schema.clone()), j.libraries.clone()))
            .unwrap_or_default();
        let found = self.browser.candidates(
            &typed,
            &statement,
            schema.as_deref(),
            &libraries,
            &mut self.database,
        );
        // Snippets by name, but not for a column after `alias.` (R-SQL-64).
        let snippets = if typed.qualifier.is_none() {
            self.snippets.completions(&typed.prefix)
        } else {
            Vec::new()
        };
        self.scripts.offer(editor::arrange(found, snippets));
    }

    /// The parameters of the routine called at the cursor, from the catalog (R-SQL-65).
    fn signatures(&mut self, ctx: &egui::Context) {
        let found = self.scripts.calling(ctx).and_then(|call| {
            let schema = self.job.as_ref().map(|j| j.schema.clone());
            let signatures = self
                .browser
                .signatures(
                    call.qualifier.as_deref(),
                    &call.name,
                    schema.as_deref(),
                    &mut self.database,
                )?
                .to_vec();
            Some((call, signatures))
        });
        self.scripts.set_signatures(found);
    }

    /// What hovering a name in the editor shows, from the schema browser (R-SQL-37).
    fn describe(&mut self) {
        let Some(((qualifier, name), statement)) = self.scripts.pointed() else {
            return;
        };
        let schema = self.job.as_ref().map(|j| j.schema.clone());
        let text = self.browser.describe(
            qualifier.as_deref(),
            &name,
            &statement,
            schema.as_deref(),
            &mut self.database,
        );
        self.scripts.describe((qualifier, name), text);
    }

    /// What was picked in the schema browser (R-SQL-34, R-SQL-35).
    fn use_pick(&mut self, pick: browser::Pick) {
        match pick {
            browser::Pick::Insert(text) => self.scripts.insert(&text),
            browser::Pick::Script { title, text, run } => {
                self.scripts.open_new(&title, &text);
                if run {
                    self.run_now(&text);
                }
            }
        }
    }

    /// A snippet into the editor at the cursor, or run (R-SQL-25).
    fn use_snippet(&mut self, used: snippets::Use) {
        match used {
            snippets::Use::Insert(sql) => self.scripts.insert(&sql),
            snippets::Use::Run(sql) => self.run_now(&sql),
        }
    }

    /// Runs the statements of `sql` as Run all does, unless a statement is running.
    fn run_now(&mut self, sql: &str) {
        if self.waiting.is_some() || !self.queue.is_empty() {
            self.error_line = Some("Wait for the statement running to end.".to_owned());
            return;
        }
        self.queue.extend(
            text::statements(sql)
                .into_iter()
                .filter_map(|r| sql.get(r).map(str::to_owned)),
        );
        self.next_statement();
    }

    /// The folded side panel: a button that opens it again (R-SQL-51).
    fn folded_side(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            if panel_button(ui, icons::CARET_LEFT, "Show History, Snippets and Database") {
                self.side_panel = SidePanel::Open;
            }
        });
    }

    fn history(&mut self, ui: &mut Ui, history: &[String]) {
        if history.is_empty() {
            theme::hint(ui, "Statements you run are listed here.");
            return;
        }
        ScrollArea::vertical()
            .id_salt("sql-history")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for statement in history.iter().rev() {
                    let line = statement.split_whitespace().collect::<Vec<_>>().join(" ");
                    let item = egui::Button::new(RichText::new(line).monospace().small())
                        .truncate()
                        .frame_when_inactive(false);
                    if ui
                        .add_sized([ui.available_width(), 20.0], item)
                        .on_hover_text(statement)
                        .clicked()
                    {
                        self.scripts.append(statement);
                    }
                }
            });
    }

    // ---------------------------------------------------------------- writing and running

    /// The editor with its Run buttons, and the results below or beside it (R-SQL-49).
    fn work(&mut self, ui: &mut Ui, around: Around<'_>) {
        self.scripts.close_pairs = around.close_pairs;
        let keys = around.keys.cloned().unwrap_or_default();
        if keys != self.scripts.keys {
            self.scripts.keys = keys;
        }
        self.keys(ui, around.files);
        self.complete();
        self.signatures(ui.ctx());
        self.describe();
        match around.results.arrangement(ui.available_width()) {
            layout::Arrangement::Stacked => self.stacked(ui, around),
            layout::Arrangement::SideBySide => self.side_by_side(ui, around),
        }
    }

    /// The editor above, the results below, a divider between them (R-SQL-50).
    fn stacked(&mut self, ui: &mut Ui, around: Around<'_>) {
        let height =
            layout::editor_height(self.editor_height, ui.available_height(), BELOW_THE_TEXT);
        let shown = ui
            .scope_builder(egui::UiBuilder::new().id_salt(WRITING), |ui| {
                self.writing(ui, height, around)
            })
            .inner;
        let (divider, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), layout::DIVIDER),
            egui::Sense::hover(),
        );
        match layout::divider(ui, divider, layout::Axis::Rows) {
            Some(layout::Drag::Moved(by)) => self.editor_height = Some(shown + by),
            Some(layout::Drag::Reset) => self.editor_height = None,
            None => {}
        }
        self.results_and_messages(ui, around.files);
    }

    /// The editor on the left, the results on the right, a divider between them (R-SQL-49).
    fn side_by_side(&mut self, ui: &mut Ui, around: Around<'_>) {
        let full = ui.available_rect_before_wrap();
        let width = layout::editor_width(self.editor_share, full.width());
        let left = egui::Rect::from_min_size(full.min, egui::vec2(width, full.height()));
        {
            let mut left_ui = ui.new_child(egui::UiBuilder::new().id_salt(WRITING).max_rect(left));
            // The text fills the column, less what was around it the last frame: the Ask row,
            // the tab strip, the find bar, the Run buttons and a line of error (R-SQL-49).
            let text = (left.height() - self.around_the_text).max(90.0);
            let height = layout::TextHeight {
                least: text,
                most: text,
            };
            let shown = self.writing(&mut left_ui, height, around);
            self.around_the_text = (left_ui.min_rect().height() - shown).max(0.0);
        }
        let divider = egui::Rect::from_min_size(
            egui::pos2(left.right(), full.top()),
            egui::vec2(layout::DIVIDER, full.height()),
        );
        match layout::divider(ui, divider, layout::Axis::Columns) {
            Some(layout::Drag::Moved(by)) if full.width() > 0.0 => {
                self.editor_share = ((width + by) / full.width()).clamp(0.0, 1.0);
            }
            Some(layout::Drag::Reset) => self.editor_share = layout::DEFAULT_SHARE,
            Some(layout::Drag::Moved(_)) | None => {}
        }
        let right = egui::Rect::from_min_max(egui::pos2(divider.right(), full.top()), full.max);
        let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right));
        self.results_and_messages(&mut right_ui, around.files);
        ui.allocate_rect(full, egui::Sense::hover());
    }

    /// The Ask row, the editor with its text `height` tall, the Run buttons and the error line;
    /// returns how high the text was drawn.
    fn writing(&mut self, ui: &mut Ui, height: layout::TextHeight, around: Around<'_>) -> f32 {
        if self.assistant.on {
            self.assistant_row(ui);
        }
        let shown = self.scripts.show(ui, height, around.files);
        ui.add_space(6.0);
        self.run_buttons(ui);
        if let Some(error) = &self.error_line {
            theme::error(ui, error);
        }
        shown
    }

    /// The Results and Messages tabs.
    fn results_and_messages(&mut self, ui: &mut Ui, saver: Option<&dyn SqlFiles>) {
        theme::segmented(
            ui,
            &mut self.tab,
            &[(Tab::Results, "Results"), (Tab::Messages, "Messages")],
        );
        ui.add_space(4.0);
        match self.tab {
            Tab::Results => self.results(ui, saver),
            Tab::Messages => self.messages(ui),
        }
    }

    /// The window's shortcuts (R-SQL-60), taken before the editor sees them: Run, Run all and
    /// Cancel here, the rest in the editor.
    fn keys(&mut self, ui: &Ui, files: Option<&dyn SqlFiles>) {
        for action in self.scripts.take_keys(ui, files) {
            match action {
                keys::SqlAction::Run => self.run_current(),
                keys::SqlAction::RunAll => self.run_all(),
                keys::SqlAction::Cancel => self.cancel(),
                _ => {}
            }
        }
    }

    fn run_buttons(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none() && self.queue.is_empty();
        ui.horizontal(|ui| {
            // R-SQL-28: says what Run does now.
            let (start, end) = self.scripts.selection();
            let label = if start < end { "Run selection" } else { "Run" };
            let run = icons::button(icons::PLAY, label)
                .shortcut_text(self.scripts.keys.label(keys::SqlAction::Run));
            if icons::add_named(ui, idle, run, label).clicked() {
                self.run_current();
            }
            let all = egui::Button::new("Run all")
                .shortcut_text(self.scripts.keys.label(keys::SqlAction::RunAll));
            if icons::add_named(ui, idle, all, "Run all").clicked() {
                self.run_all();
            }
            let cancel = icons::button(icons::STOP, "Cancel").shortcut_text("Esc");
            if icons::add_named(ui, !idle, cancel, "Cancel").clicked() {
                self.cancel();
            }
            if !idle {
                ui.spinner();
                theme::hint(ui, "Running…");
            }
            theme::button_row(ui, |ui| {
                // R-SQL-25: what Run would run, kept as a snippet.
                let current = self.scripts.current_statement();
                let save = icons::button(icons::BOOKMARK_SIMPLE, "Save as snippet…");
                let clicked = icons::add_named(ui, current.is_some(), save, "Save as snippet…")
                    .on_hover_text("The selection, or the statement under the cursor")
                    .clicked();
                if clicked && let Some(sql) = current {
                    self.snippets.save_as(&sql);
                    self.side = Side::Snippets;
                }
            });
        });
    }

    /// The selection, or the statement under the cursor (R-SQL-7).
    fn run_current(&mut self) {
        if self.waiting.is_some() || !self.queue.is_empty() {
            return;
        }
        if let Some(statement) = self.scripts.current_statement() {
            self.queue.push_back(statement);
            self.next_statement();
        }
    }

    /// Every statement in the editor, in order, stopping at the first error.
    fn run_all(&mut self) {
        if self.waiting.is_some() || !self.queue.is_empty() {
            return;
        }
        let script = self.scripts.text();
        let statements: Vec<String> = text::statements(script)
            .into_iter()
            .filter_map(|r| script.get(r).map(str::to_owned))
            .collect();
        self.queue.extend(statements);
        self.next_statement();
    }

    /// Runs the next statement queued, after asking when it changes much (R-SQL-9).
    fn next_statement(&mut self) {
        if self.waiting.is_some() || self.confirm.is_some() || self.parameters.is_some() {
            return;
        }
        let Some(statement) = self.queue.pop_front() else {
            return;
        };
        if let Some(risk) = text::risk(&statement) {
            self.confirm = Some((statement, risk));
            return;
        }
        self.start(statement);
    }

    /// Runs `statement`, after asking for its markers' values when it has any (R-SQL-40).
    fn start(&mut self, statement: String) {
        self.error_line = None;
        if text::cl_command(&statement).is_none()
            && let Some(markers) = params::markers(&statement)
        {
            let remembered = self.param_values.get(&statement);
            self.parameters = Some(parameters::ParamForm::new(statement, markers, remembered));
            return;
        }
        let run = self.run_request(&statement);
        self.send(run, Waiting::Run(statement));
    }

    /// The request that runs `statement`, with the rows at a time and the timeout (R-SQL-18).
    /// A `CL:` command runs through `QSYS2.QCMDEXC` (R-SQL-30).
    fn run_request(&self, statement: &str) -> Request {
        let sql = text::cl_command(statement).map_or_else(|| statement.to_owned(), text::cl_call);
        Request::Run {
            sql,
            max_rows: self.rows,
            timeout: self.timeout,
            params: Vec::new(),
        }
    }

    /// The parameter form while open: Run sends the marked statement with the values given
    /// and remembers them; Cancel stops what was queued (R-SQL-40).
    fn parameters_dialog(&mut self, ctx: &egui::Context) {
        let Some(form) = &mut self.parameters else {
            return;
        };
        let Some(outcome) = form.show(ctx) else {
            return;
        };
        let Some(form) = self.parameters.take() else {
            return;
        };
        match outcome {
            parameters::Outcome::Run(values) => {
                self.param_values
                    .insert(form.statement.clone(), form.values.clone());
                let run = Request::Run {
                    sql: form.markers.sql,
                    max_rows: self.rows,
                    timeout: self.timeout,
                    params: values,
                };
                self.send(run, Waiting::Run(form.statement));
            }
            parameters::Outcome::Cancel => self.queue.clear(),
        }
    }

    fn cancel(&mut self) {
        self.queue.clear();
        if let Some(runner) = &self.runner
            && self.waiting.is_some()
        {
            runner.cancel();
        }
    }

    fn send(&mut self, request: Request, waiting: Waiting) {
        if matches!(waiting, Waiting::Run(_)) {
            self.run_started = Some(std::time::Instant::now());
        }
        if let Some(runner) = &mut self.runner {
            let tag = runner.send(request);
            self.waiting = Some((tag, waiting));
        }
    }

    // ---------------------------------------------------------------- results

    fn results(&mut self, ui: &mut Ui, saver: Option<&dyn SqlFiles>) {
        let idle = self.waiting.is_none();
        let asked = self.results.show(ui, idle, &mut self.header, saver);
        if let Some(results::Ask::Fetch(cursor)) = asked {
            let fetch = Request::Fetch {
                cursor,
                max_rows: self.rows,
            };
            self.send(fetch, Waiting::Fetch(cursor));
        }
    }

    // ---------------------------------------------------------------- answers

    /// Takes what the helper answered since the last frame.
    fn read_events(&mut self) {
        while let Some(event) = self.runner.as_ref().and_then(Runner::try_event) {
            self.handle(event);
        }
        self.database.read_events();
        for pick in self.browser.receive(self.database.take_answers()) {
            self.use_pick(pick);
        }
    }

    fn handle(&mut self, event: Event) {
        let Some((tag, waiting)) = self.waiting.take() else {
            return;
        };
        if event.tag != tag && event.tag != 0 {
            self.waiting = Some((tag, waiting));
            return;
        }
        match waiting {
            Waiting::Hello => self.on_hello(event),
            Waiting::Connect => self.on_connect(event),
            Waiting::Run(statement) => self.on_run(&statement, event),
            Waiting::Explain { index, id, file } => self.on_explain(index, &id, file, event),
            Waiting::JobLog => self.on_job_log(event),
            Waiting::Fetch(cursor) => match event
                .result
                .and_then(parley_core::sql::protocol::Answer::into_result::<MoreRows>)
            {
                Ok(more) => self.results.append(cursor, more.rows, more.more),
                Err(e) => self.fail(e.to_string()),
            },
            Waiting::Autocommit(on) => match event
                .result
                .and_then(parley_core::sql::protocol::Answer::into_result::<Done>)
            {
                Ok(_) => self.autocommit = on,
                Err(e) => self.fail(e.to_string()),
            },
            Waiting::Job => self.on_job(event),
            Waiting::Commit | Waiting::Rollback => {
                let done = if waiting == Waiting::Commit {
                    "Committed."
                } else {
                    "Rolled back."
                };
                match event
                    .result
                    .and_then(parley_core::sql::protocol::Answer::into_result::<Done>)
                {
                    Ok(_) => {
                        self.uncommitted = false;
                        self.messages.push((false, done.to_owned()));
                    }
                    Err(e) => self.fail(e.to_string()),
                }
                if self.closing == Closing::AfterCommit {
                    self.closing = if self.uncommitted {
                        Closing::Asking
                    } else {
                        Closing::Closed
                    };
                }
            }
        }
        self.read_job_when_idle();
    }

    fn on_hello(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Hello>)
        {
            Ok(hello) if hello.protocol == protocol::PROTOCOL => {
                if let Some(connect) = self.pending_connect.take() {
                    self.send(connect, Waiting::Connect);
                }
            }
            Ok(hello) => self.back_to_sign_in(format!(
                "The SQL helper speaks protocol {}, Parley {}: the installation is mixed.",
                hello.protocol,
                protocol::PROTOCOL
            )),
            Err(e) => self.back_to_sign_in(e.to_string()),
        }
    }

    fn on_connect(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Connected>)
        {
            Ok(connected) => {
                self.messages.push((
                    false,
                    format!(
                        "Connected as {} to {} (job {}).",
                        connected.user, self.config.host, connected.job
                    ),
                ));
                self.connected = Some(connected);
                self.stage = Stage::Ready;
                self.job_stale = true;
                if let Some(connect) = self.pending_catalog.take() {
                    self.database.sign_on(connect);
                }
            }
            Err(e) => self.back_to_sign_in(format!(
                "{e} Not tried again: check the user and password first."
            )),
        }
    }

    fn on_run(&mut self, statement: &str, event: Event) {
        self.ran.push(statement.to_owned());
        if text::may_change_job(statement) {
            self.job_stale = true;
        }
        let short = statement.split_whitespace().collect::<Vec<_>>().join(" ");
        match event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Outcome>)
        {
            Ok(Outcome::Rows {
                cursor,
                columns,
                rows,
                more,
            }) => {
                self.note_outcome(
                    statement,
                    gutter::Outcome::Rows {
                        count: rows.len(),
                        more,
                    },
                );
                self.messages.push((
                    false,
                    format!(
                        "{short}: {} rows{}",
                        rows.len(),
                        if more { ", more to fetch" } else { "" }
                    ),
                ));
                self.results
                    .add(statement, ResultView::new(columns, rows, more, cursor));
                self.tab = Tab::Results;
            }
            Ok(Outcome::Count { .. }) if text::cl_command(statement).is_some() => {
                self.note_outcome(statement, gutter::Outcome::Done);
                self.messages.push((false, format!("{short}: done")));
                if self.queue.is_empty() {
                    self.tab = Tab::Messages;
                }
            }
            Ok(Outcome::Count { count }) => {
                self.note_outcome(statement, gutter::Outcome::Changed(count));
                self.messages
                    .push((false, format!("{short}: {count} rows changed")));
                if !self.autocommit {
                    self.uncommitted = true;
                }
                if self.queue.is_empty() {
                    self.tab = Tab::Messages;
                }
            }
            Err(e) => {
                self.note_outcome(statement, gutter::Outcome::Failed(e.to_string()));
                self.queue.clear();
                self.fail(format!("{short}: {e}"));
                return;
            }
        }
        self.next_statement();
    }

    /// Keeps how `statement` ended for the editor's gutter (R-SQL-62).
    fn note_outcome(&mut self, statement: &str, outcome: gutter::Outcome) {
        let took = self
            .run_started
            .take()
            .map(|t| t.elapsed())
            .unwrap_or_default();
        self.scripts.outcomes.note(statement, outcome, took);
    }

    fn fail(&mut self, message: String) {
        self.messages.push((true, message.clone()));
        self.error_line = Some(message);
    }

    fn back_to_sign_in(&mut self, message: String) {
        self.runner = None;
        self.pending_connect = None;
        // The first sign-on failed: the second does not try the same password.
        self.pending_catalog = None;
        self.database.abandon();
        self.stage = Stage::SignIn;
        self.error_line = Some(message);
    }

    // ---------------------------------------------------------------- questions

    /// R-SQL-9: names the statement and what it does; Run goes on, Cancel stops the queue.
    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some((statement, risk)) = self.confirm.clone() else {
            return;
        };
        let (close, (cancel, run)) = dialog::dialog(
            ctx,
            "sql-confirm",
            "Run this statement?",
            460.0,
            |ui| {
                ui.label(RichText::new(&statement).monospace());
                ui.add_space(6.0);
                theme::warning(ui, format!("It {}.", risk.describe()));
                theme::hint(
                    ui,
                    "Parley only reads the statement's first words: this guards against slips, it does not check what the statement does.",
                );
                dialog::actions(ui, theme::danger_button(ui, "Run"), true)
            },
        );
        if run {
            self.confirm = None;
            self.start(statement);
        } else if close || cancel {
            self.confirm = None;
            self.queue.clear();
        }
    }

    /// R-SQL-5: closing with changes not committed.
    fn closing_dialog(&mut self, ctx: &egui::Context) {
        if self.closing != Closing::Asking {
            return;
        }
        let (close, choice) = dialog::dialog(
            ctx,
            "sql-closing",
            "Changes are not committed",
            420.0,
            |ui| {
                ui.label("Commit them, or roll them back, before the SQL window closes?");
                ui.add_space(12.0);
                theme::button_row(ui, |ui| {
                    if ui.add(theme::primary_button(ui, "Commit")).clicked() {
                        Some(Waiting::Commit)
                    } else if ui.button("Roll back").clicked() {
                        Some(Waiting::Rollback)
                    } else if ui.button("Cancel").clicked() {
                        Some(Waiting::Hello)
                    } else {
                        None
                    }
                })
            },
        );
        match choice {
            Some(Waiting::Commit) => {
                self.closing = Closing::AfterCommit;
                self.send(Request::Commit, Waiting::Commit);
            }
            Some(Waiting::Rollback) => {
                self.closing = Closing::AfterCommit;
                self.send(Request::Rollback, Waiting::Rollback);
            }
            Some(_) => self.closing = Closing::Open,
            None if close => self.closing = Closing::Open,
            None => {}
        }
    }

    /// Ends the helper: the connection closes with it.
    fn finish(&mut self) {
        if let Some(mut runner) = self.runner.take() {
            runner.send(Request::Close);
        }
        self.database.close();
    }
}

#[cfg(test)]
mod tests {
    use parley_core::sql::protocol::Answer;

    use super::*;

    pub(super) fn window() -> SqlWindow {
        let config = SessionConfig {
            host: "ibmi".to_owned(),
            ..SessionConfig::default()
        };
        let mut w = SqlWindow::open(
            &config,
            PathBuf::from("java"),
            HelperFiles::in_folder(std::path::Path::new("/none")),
            "usr",
            &[],
        );
        w.stage = Stage::Ready;
        w
    }

    /// The window as if the helper answered `json` to what it waits for.
    pub(super) fn answer(w: &mut SqlWindow, json: &str) {
        let tag = w.waiting.as_ref().map_or(0, |(tag, _)| *tag);
        w.handle(Event {
            tag,
            result: Answer::parse(json),
        });
    }

    /// The window waiting for `statement`, as if it had sent it.
    pub(super) fn running(w: &mut SqlWindow, statement: &str) {
        w.waiting = Some((1, Waiting::Run(statement.to_owned())));
    }

    #[test]
    fn rows_fill_the_grid_and_go_into_the_history_r_sql_8() {
        let mut w = window();
        w.tab = Tab::Messages;
        running(&mut w, "SELECT NAME FROM T");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false}],"rows":[["æøå"],[null]],"more":true}}"#,
        );
        let view = w.results.shown_view().expect("a grid");
        assert_eq!(view.rows.len(), 2);
        assert_eq!(view.cursor, Some(3), "more to fetch");
        assert_eq!(w.tab, Tab::Results);
        assert_eq!(w.take_ran(), ["SELECT NAME FROM T"]);
        assert!(w.waiting.is_none());
    }

    /// R-SQL-62: each run leaves its outcome for the gutter, an error too.
    #[test]
    fn a_run_leaves_its_outcome_for_the_gutter_r_sql_62() {
        let mut w = window();
        running(&mut w, "SELECT NAME FROM T");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false}],"rows":[["æøå"],[null]],"more":true}}"#,
        );
        running(&mut w, "DELETE FROM T WHERE A = 'å'");
        answer(
            &mut w,
            r#"{"id":1,"error":{"message":"T i *LIBL findes ikke.","sqlstate":"42704","sqlcode":-204}}"#,
        );
        let outcomes = format!("{:?}", w.scripts.outcomes);
        assert!(
            outcomes.contains("Rows { count: 2, more: true }"),
            "{outcomes}"
        );
        assert!(outcomes.contains("findes ikke"), "{outcomes}");
    }

    /// R-SQL-62: the ▶ on a statement's first line selects that statement and runs it.
    #[test]
    fn the_play_mark_in_the_gutter_runs_its_statement_r_sql_62() {
        use egui_kittest::kittest::Queryable;

        let mut w = window().signed_on_for_tests();
        w.scripts.set_text("VALUES 1;\n\nVALUES 'æøå';\n", 0);
        let mut harness = drawn(w, Vec::new());
        harness.run();
        harness.get_by_label("Run the statement on line 3").click();
        harness.run();
        let script = "VALUES 1;\n\nVALUES 'æøå';\n";
        let start = script.find("VALUES 'æøå'").expect("there");
        assert_eq!(
            harness.state().scripts.selection(),
            (start, start + "VALUES 'æøå'".len())
        );
        assert!(harness.state().run_started.is_some(), "it was run");
    }

    #[test]
    fn a_change_without_autocommit_is_uncommitted_and_closing_asks_r_sql_5() {
        let mut w = window();
        w.autocommit = false;
        running(&mut w, "UPDATE T SET A = 1 WHERE B = 2");
        answer(&mut w, r#"{"id":1,"ok":{"kind":"count","count":3}}"#);
        assert!(w.uncommitted);
        assert!(w.messages.iter().any(|(_, m)| m.contains("3 rows changed")));
        w.request_close();
        assert_eq!(w.closing, Closing::Asking);
    }

    #[test]
    fn closing_with_everything_committed_closes_at_once_r_sql_5() {
        let mut w = window();
        w.request_close();
        assert_eq!(w.closing, Closing::Closed);
    }

    #[test]
    fn an_error_stops_run_all_and_shows_the_red_line_r_sql_7() {
        let mut w = window();
        w.queue
            .extend(["VALUES 2".to_owned(), "VALUES 3".to_owned()]);
        running(&mut w, "SELECT * FROM NOPE");
        answer(
            &mut w,
            r#"{"id":1,"error":{"message":"NOPE in *LIBL type *FILE not found.","sqlstate":"42704","sqlcode":-204}}"#,
        );
        assert!(w.queue.is_empty(), "the rest is not run");
        let line = w.error_line.as_deref().expect("an error line");
        assert!(line.contains("SQLSTATE 42704"), "{line}");
        assert_eq!(
            w.take_ran(),
            ["SELECT * FROM NOPE"],
            "failed statements are in the history too"
        );
    }

    #[test]
    fn a_bulk_change_waits_for_confirmation_r_sql_9() {
        let mut w = window();
        let script = "SELECT 1 FROM T; DELETE FROM T";
        w.scripts.set_text(script, script.len());
        w.run_current();
        let (statement, risk) = w.confirm.clone().expect("asked");
        assert_eq!(statement, "DELETE FROM T");
        assert_eq!(risk, Risk::DeleteWithoutWhere);
        assert!(w.waiting.is_none(), "nothing sent before the answer");
    }

    #[test]
    fn the_ask_row_shows_only_while_the_assistant_add_on_is_on_r_add_3() {
        use egui_kittest::kittest::Queryable;
        let mut harness = drawn(window(), Vec::new());
        harness.run_steps(2);
        assert!(
            harness.query_by_label("Ask").is_none(),
            "off: nothing of it shows"
        );
        let config = parley_core::sql::assistant::AssistantConfig::default();
        harness.state_mut().set_assistant(true, &config, None);
        harness.run_steps(2);
        assert!(harness.query_by_label("Ask").is_some());
    }

    #[test]
    fn a_statement_with_markers_asks_for_values_before_it_runs_r_sql_40() {
        let mut w = window();
        w.scripts
            .set_text("SELECT * FROM T WHERE A = :navn; VALUES 'æøå'", 0);
        w.run_all();
        assert!(
            w.waiting.is_none(),
            "nothing sent before the values are given"
        );
        let form = w.parameters.as_ref().expect("the form");
        assert_eq!(form.markers.sql, "SELECT * FROM T WHERE A = ?");
        assert_eq!(w.queue, ["VALUES 'æøå'"], "the next waits for the form");
        w.next_statement();
        assert_eq!(w.queue.len(), 1, "still waiting while the form is open");
        // A CL command is never read for markers.
        let mut w = window();
        w.start("CL: SNDMSG MSG('?') TOUSR(:X)".to_owned());
        assert!(w.parameters.is_none());
    }

    #[test]
    fn run_all_queues_every_statement_in_order_r_sql_7() {
        let mut w = window();
        w.scripts.set_text("VALUES 1;\nVALUES 'æøå';\n", 0);
        w.run_all();
        // Without a helper nothing is sent; the first statement left the queue to be run.
        assert_eq!(w.queue, ["VALUES 'æøå'"]);
    }

    #[test]
    fn a_refused_sign_on_goes_back_to_the_form_r_sql_6() {
        let mut w = window();
        w.stage = Stage::Connecting;
        w.waiting = Some((1, Waiting::Connect));
        w.pending_catalog = Some(Request::Hello);
        answer(
            &mut w,
            r#"{"id":1,"error":{"message":"Password is not correct for user profile.","sqlstate":"08004","sqlcode":-99999}}"#,
        );
        assert_eq!(w.stage, Stage::SignIn);
        let line = w.error_line.as_deref().expect("why");
        assert!(line.contains("Not tried again"), "{line}");
        // R-SQL-33: the second connection does not try the same password.
        assert!(w.pending_catalog.is_none());
        assert_eq!(w.database.state, database::State::Off);
    }

    #[test]
    fn the_password_leaves_its_field_even_when_the_helper_cannot_start_r_sql_6() {
        let config = SessionConfig::default();
        let files = HelperFiles::in_folder(std::path::Path::new("/none"));
        let mut w = SqlWindow::open(&config, PathBuf::from("/no/such/java"), files, "usr", &[]);
        "TopSecret9".clone_into(&mut w.password);
        w.connect(&egui::Context::default());
        assert_eq!(w.password, "", "not kept in the field");
        assert_eq!(w.stage, Stage::SignIn);
        let line = w.error_line.as_deref().expect("why");
        assert!(!line.contains("TopSecret9"), "{line}");
    }

    #[test]
    fn character_positions_become_byte_offsets_r_sql_7() {
        assert_eq!(editor::byte_at("æøå;", 3), 6);
        assert_eq!(editor::byte_at("abc", 9), 3);
    }

    /// The window drawn alone, as in a test of the main window (embedded: kittest has one).
    pub(super) fn drawn(
        w: SqlWindow,
        history: Vec<String>,
    ) -> egui_kittest::Harness<'static, SqlWindow> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(1100.0, 760.0))
            .with_max_steps(16)
            .build_ui_state(
                move |ui, w: &mut SqlWindow| {
                    crate::test_support::install_fonts(ui);
                    let around = Around {
                        history: &history,
                        ..Around::default()
                    };
                    w.show_in(ui, around);
                },
                w,
            )
    }

    #[test]
    fn the_sign_in_form_asks_user_and_password_and_warns_without_tls_r_sql_6() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        w.stage = Stage::SignIn;
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("User");
        harness.get_by_label("Password");
        harness.get_by_label_contains("Not encrypted");
        // Without a password Connect does nothing: no helper is started.
        harness.get_by_label("Connect").click();
        harness.run();
        assert_eq!(harness.state().stage, Stage::SignIn);
        assert!(harness.state().runner.is_none());
    }

    #[test]
    fn the_ready_window_draws_results_history_and_buttons_and_keeps_its_place_r_sql_8() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        running(&mut w, "SELECT NAME FROM T");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false},{"name":"AMOUNT","type":"DECIMAL","length":31,"scale":6,"nullable":true,"numeric":true}],"rows":[["æøå","12345678901234567890.123456"],[null,"-0.50"]],"more":true}}"#,
        );
        let mut harness = drawn(w, vec!["SELECT NAME FROM T".to_owned()]);
        for label in [
            "Run",
            "Run all",
            "Cancel",
            "Results",
            "Messages",
            "Fetch more",
            "Copy all",
            "History",
            "Snippets",
            "Save as snippet…",
        ] {
            harness.get_by_label(label);
        }
        harness.get_by_label("æøå");
        harness.get_by_label("12345678901234567890.123456");
        let run = harness.get_by_label("Run").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Run").rect(), run);
    }

    /// Draws a signed-on window `width` wide with its results placed as `results` asks.
    fn drawn_with_results(
        width: f32,
        results: layout::ResultsPlace,
    ) -> egui_kittest::Harness<'static, SqlWindow> {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(width, 760.0))
            .with_max_steps(16)
            .build_ui_state(
                move |ui, w: &mut SqlWindow| {
                    crate::test_support::install_fonts(ui);
                    let around = Around {
                        results,
                        ..Around::default()
                    };
                    // In the harness's whole width, as Parley SQL draws it.
                    w.show_in(ui, around);
                },
                window().signed_on_for_tests(),
            );
        harness.run();
        harness
    }

    /// R-SQL-49: below, the Results tab is under the Run button; beside, to its right, and the
    /// divider between them stays put over a few frames.
    #[test]
    fn the_results_go_below_or_beside_the_editor_as_the_settings_ask_r_sql_49() {
        use egui_kittest::kittest::Queryable;

        let harness = drawn_with_results(1100.0, layout::ResultsPlace::Below);
        let run = harness.get_by_label("Run").rect();
        let tab = harness.get_by_label("Results").rect();
        assert!(tab.top() > run.bottom(), "below: {tab:?} under {run:?}");
        let divider = harness.get_by_label("Editor and results divider").rect();
        assert!(divider.width() > divider.height(), "across: {divider:?}");

        let mut harness = drawn_with_results(1100.0, layout::ResultsPlace::Beside);
        let run = harness.get_by_label("Run").rect();
        let tab = harness.get_by_label("Results").rect();
        assert!(tab.left() > run.right(), "beside: {tab:?} right of {run:?}");
        assert!(tab.top() < run.top(), "at the top: {tab:?}, {run:?}");
        let divider = harness.get_by_label("Editor and results divider").rect();
        harness.run_steps(3);
        assert_eq!(
            harness.get_by_label("Editor and results divider").rect(),
            divider
        );
    }

    /// R-SQL-49 (a fix): when Automatic puts the results below instead of beside, the editor
    /// keeps the focus and what is typed still goes into it.
    #[test]
    fn the_editor_keeps_the_keyboard_when_the_results_move_r_sql_49() {
        use egui_kittest::kittest::Queryable;

        let mut harness = drawn_with_results(1600.0, layout::ResultsPlace::Automatic);
        harness
            .get_by_role(egui::accesskit::Role::MultilineTextInput)
            .focus();
        harness.run();
        harness.event(egui::Event::Text("a".to_owned()));
        harness.run();
        harness.set_size(egui::vec2(980.0, 760.0));
        harness.run();
        harness.event(egui::Event::Text("b".to_owned()));
        harness.run();
        assert_eq!(harness.state().scripts.text(), "ab");
    }

    /// Drags the editor and results divider by `by`.
    fn drag_divider(harness: &mut egui_kittest::Harness<'static, SqlWindow>, by: egui::Vec2) {
        use egui_kittest::kittest::Queryable;

        let from = harness
            .get_by_label("Editor and results divider")
            .rect()
            .center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        harness.event(egui::Event::PointerMoved(from));
        harness.event(button(from, true));
        harness.run();
        for step in 1..=4 {
            #[expect(clippy::cast_precision_loss, reason = "a step of four")]
            harness.event(egui::Event::PointerMoved(from + by * (step as f32 / 4.0)));
            harness.run();
        }
        harness.event(button(from + by, false));
        harness.run();
    }

    /// R-SQL-50: below, dragging the divider down makes the editor taller and the results
    /// start lower; the height lasts over frames.
    #[test]
    fn dragging_the_divider_below_changes_the_editor_height_r_sql_50() {
        use egui_kittest::kittest::Queryable;

        let mut harness = drawn_with_results(1100.0, layout::ResultsPlace::Below);
        let before = harness.get_by_label("Results").rect().top();
        drag_divider(&mut harness, egui::vec2(0.0, 80.0));
        harness.run_steps(3);
        let after = harness.get_by_label("Results").rect().top();
        assert!(
            (after - before - 80.0).abs() < 4.0,
            "moved from {before} to {after}"
        );
        assert!(harness.state().editor_height.is_some());
    }

    /// R-SQL-50: dragging past the bottom leaves the results their least height.
    #[test]
    fn the_editor_cannot_push_the_results_out_of_the_window_r_sql_50() {
        use egui_kittest::kittest::Queryable;

        let mut harness = drawn_with_results(1100.0, layout::ResultsPlace::Below);
        drag_divider(&mut harness, egui::vec2(0.0, 2000.0));
        harness.run_steps(3);
        let tab = harness.get_by_label("Results").rect();
        assert!(tab.bottom() < 760.0 - 100.0, "results shown: {tab:?}");
    }

    /// R-SQL-51: the side panel folds to a strip and opens again, and the editor gets the room.
    #[test]
    fn the_side_panel_folds_away_and_opens_again_r_sql_51() {
        use egui_kittest::kittest::Queryable;

        let mut harness = drawn_with_results(1100.0, layout::ResultsPlace::Below);
        let wide = harness
            .get_by_label("Editor and results divider")
            .rect()
            .width();
        harness.get_by_label("Hide the side panel").click();
        harness.run();
        assert!(harness.query_by_label("History").is_none());
        let wider = harness
            .get_by_label("Editor and results divider")
            .rect()
            .width();
        assert!(wider > wide + 150.0, "{wide} then {wider}");
        harness
            .get_by_label("Show History, Snippets and Database")
            .click();
        harness.run();
        harness.get_by_label("History");
        harness.get_by_label("Hide the side panel");
    }

    /// R-SQL-49: beside, the editor fills the column: the Run buttons are at its foot, and they
    /// stay there over frames.
    #[test]
    fn beside_the_editor_takes_the_full_height_r_sql_49() {
        use egui_kittest::kittest::Queryable;

        let mut harness = drawn_with_results(1100.0, layout::ResultsPlace::Beside);
        harness.run_steps(2);
        let divider = harness.get_by_label("Editor and results divider").rect();
        let run = harness.get_by_label("Run").rect();
        assert!(
            divider.bottom() - run.bottom() < 30.0,
            "Run at {run:?}, column to {}",
            divider.bottom()
        );
        harness.run_steps(3);
        assert_eq!(harness.get_by_label("Run").rect(), run);
    }

    /// R-SQL-49: automatic puts the results beside the editor only in a wide window.
    #[test]
    fn automatic_results_go_beside_in_a_wide_window_r_sql_49() {
        use egui_kittest::kittest::Queryable;

        // Below, the divider lies across; beside, it stands up.
        let narrow = drawn_with_results(980.0, layout::ResultsPlace::Automatic);
        let divider = narrow.get_by_label("Editor and results divider").rect();
        assert!(divider.width() > divider.height(), "below: {divider:?}");
        let wide = drawn_with_results(1600.0, layout::ResultsPlace::Automatic);
        let divider = wide.get_by_label("Editor and results divider").rect();
        assert!(divider.height() > divider.width(), "beside: {divider:?}");
    }

    #[test]
    fn the_snippets_tab_lists_the_starters_and_runs_one_r_sql_25() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        running(&mut w, "VALUES 1");
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("Snippets").click();
        // The spinner keeps repainting while a statement runs.
        harness.run_steps(3);
        harness.get_by_label("Run System status").click();
        // The spinner keeps repainting while a statement runs.
        harness.run_steps(3);
        assert_eq!(
            harness.state().error_line.as_deref(),
            Some("Wait for the statement running to end."),
            "the click reached the window, which runs one statement at a time"
        );
    }

    #[test]
    fn a_snippet_run_queues_its_statements_and_asks_before_a_delete_r_sql_25() {
        let mut w = window();
        w.use_snippet(snippets::Use::Run(
            "VALUES 'æøå';\nDELETE FROM T".to_owned(),
        ));
        // Without a helper nothing is sent; the first left the queue, the DELETE waits its turn.
        assert_eq!(w.queue, ["DELETE FROM T"]);
    }

    #[test]
    fn a_snippet_goes_in_at_the_cursor_and_the_statement_is_offered_to_save_r_sql_25() {
        let mut w = window();
        w.scripts.set_text("VALUES 'æøå';\n\nSELECT 2", 3);
        assert_eq!(
            w.scripts.current_statement().as_deref(),
            Some("VALUES 'æøå'")
        );
        w.use_snippet(snippets::Use::Insert("X ".to_owned()));
        assert_eq!(w.scripts.text(), "VALX UES 'æøå';\n\nSELECT 2");
        w.scripts.select(0, 0);
        w.use_snippet(snippets::Use::Insert("-- ".to_owned()));
        assert!(w.scripts.text().starts_with("-- VALX"));
    }

    #[test]
    fn a_cl_command_runs_through_qcmdexc_and_is_kept_as_written_r_sql_30() {
        let mut w = window();
        assert_eq!(
            w.run_request("CL: SNDMSG MSG('æøå') TOUSR(QSYSOPR)"),
            Request::Run {
                sql: "CALL QSYS2.QCMDEXC('SNDMSG MSG(''æøå'') TOUSR(QSYSOPR)')".to_owned(),
                max_rows: protocol::FIRST_ROWS,
                timeout: 0,
                params: Vec::new(),
            }
        );
        w.autocommit = false;
        running(&mut w, "CL: ADDLIBLE MYLIB");
        answer(&mut w, r#"{"id":1,"ok":{"kind":"count","count":0}}"#);
        assert_eq!(w.take_ran(), ["CL: ADDLIBLE MYLIB"]);
        assert_eq!(
            w.messages.last().map(|(_, m)| m.as_str()),
            Some("CL: ADDLIBLE MYLIB: done")
        );
        assert!(!w.uncommitted, "a command changes no rows to commit");
    }

    #[test]
    fn the_rows_at_a_time_and_the_timeout_go_with_each_statement_r_sql_18() {
        let w = window().with_limits(250, 15);
        assert_eq!(
            w.run_request("VALUES 1"),
            Request::Run {
                sql: "VALUES 1".to_owned(),
                max_rows: 250,
                timeout: 15,
                params: Vec::new(),
            }
        );
        let plain = window();
        assert_eq!(
            plain.run_request("VALUES 1"),
            Request::Run {
                sql: "VALUES 1".to_owned(),
                max_rows: protocol::FIRST_ROWS,
                timeout: 0,
                params: Vec::new(),
            },
            "the defaults: 1000 rows, no timeout"
        );
    }

    #[test]
    fn enter_on_a_selected_row_shows_its_values_one_under_another_r_sql_15() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        running(&mut w, "SELECT NAME FROM T");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"NAME","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false},{"name":"NOTE","type":"VARCHAR","length":10,"scale":0,"nullable":true,"numeric":false}],"rows":[["æøå",null],["b","c"]],"more":false}}"#,
        );
        let mut harness = drawn(w, Vec::new());
        assert!(harness.query_by_label_contains("Row 1 of").is_none());
        harness.get_by_label("1").click();
        harness.run();
        let selected = harness
            .state()
            .results
            .shown_view()
            .map(ResultView::selected_count);
        assert_eq!(selected, Some(1), "a click on the row's number selects it");
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.get_by_label("Row 1 of 2");
        harness.get_by_label("Copy NAME");
        harness.get_by_label("Next row").click();
        harness.run();
        harness.get_by_label("Row 2 of 2");
    }

    #[test]
    fn ctrl_f_opens_the_find_bar_and_esc_closes_it_without_cancelling_r_sql_23() {
        use egui_kittest::kittest::Queryable;
        let mut harness = drawn(window(), Vec::new());
        assert!(harness.query_by_label("Match case").is_none());
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
        harness.run();
        harness.get_by_label("Match case");
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(harness.query_by_label("Match case").is_none());
        assert!(!harness.state().scripts.takes_escape());
    }

    /// R-SQL-60: with Find moved to F3, F3 opens the find bar and Ctrl+F no longer does; the
    /// Run button shows the key in use.
    #[test]
    fn the_window_uses_the_keys_from_the_settings_r_sql_60() {
        use egui_kittest::kittest::Queryable;

        let mut overrides = keys::Overrides::new();
        overrides.insert("find".to_owned(), vec!["F3".to_owned()]);
        overrides.insert("run".to_owned(), vec!["F5".to_owned()]);
        let keys = keys::SqlKeys::new(&overrides);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1100.0, 760.0))
            .build_ui_state(
                move |ui, w: &mut SqlWindow| {
                    crate::test_support::install_fonts(ui);
                    let around = Around {
                        keys: Some(&keys),
                        ..Around::default()
                    };
                    w.show_in(ui, around);
                },
                window().signed_on_for_tests(),
            );
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
        harness.run();
        assert!(harness.query_by_label("Match case").is_none());
        harness.key_press(egui::Key::F3);
        harness.run();
        harness.get_by_label("Match case");
        assert_eq!(
            harness.state().scripts.keys.label(keys::SqlAction::Run),
            "F5",
            "what the Run button shows"
        );
    }

    #[test]
    fn ctrl_t_adds_a_script_tab_r_sql_20() {
        use egui_kittest::kittest::Queryable;
        let mut harness = drawn(window(), Vec::new());
        harness.get_by_label("Script 1");
        assert!(harness.query_by_label("Script 2").is_none());
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
        harness.run();
        harness.get_by_label("Script 2");
        let (_, saved) = harness.state().scripts();
        assert_eq!(saved.len(), 2, "both kept for the session");
    }

    #[test]
    fn run_says_run_selection_while_text_is_selected_r_sql_28() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        w.scripts.set_text("VALUES 1; VALUES 2", 0);
        let harness = drawn(w, Vec::new());
        harness.get_by_label("Run");
        let mut w = window();
        w.scripts.set_text("VALUES 1; VALUES 2", 0);
        w.scripts.select(0, 8);
        let harness = drawn(w, Vec::new());
        harness.get_by_label("Run selection");
    }

    #[test]
    fn the_column_heading_shows_by_default_and_names_switch_it_r_sql_29() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        running(&mut w, "SELECT * FROM MSGSUDTA.REGP");
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":3,"columns":[{"name":"REGSEQ","type":"DECIMAL","length":9,"scale":0,"nullable":false,"numeric":true,"field":"REGSEQ","schema":"MSGSUDTA","table":"REGP","heading":"&Registration.Sequence","text":"&Registration.Sequence"}],"rows":[["1"]],"more":false}}"#,
        );
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("&Registration.Sequence");
        assert!(harness.query_by_label("REGSEQ").is_none());
        harness.get_by_label("Names").click();
        harness.run();
        harness.get_by_label("REGSEQ");
        assert_eq!(
            harness.state().header,
            grid::HeaderMode::Names,
            "kept for the settings"
        );
    }
}
