//! Parley SQL's user interface (D26, D36), on Parley's shared look (`parley_ui_egui`'s theme,
//! dialogs, icons and settings frame) and Parley SQL's engine (`parley_sql_core`). Nothing of
//! Parley's terminal is used here, and Parley's UI uses nothing of this crate.
//!
//! The SQL window (R-SQL-6..9, D20): the operator signs on to a host, writes SQL, runs it
//! through the SQL helper and reads the results. The helper runs on a thread of its own
//! (`parley_sql_core::runner`), so the window never waits for the host. [`app`] is the program
//! around it, Parley SQL (R-SQL-45).

mod answers;
mod app;
mod assistant;
mod browser;
mod chart;
mod database;
mod download;
mod editor;
mod grid;
mod history;
mod hosting;
mod keys;
mod layout;
mod note;
mod parameters;
mod picks;
mod questions;
mod results;
mod run;
mod settings;
mod side;
mod sign_in;
mod snippets;
mod system;
mod tools;
mod top_bar;
mod upload;

pub use app::{ParleySqlUi, SAVE_EVERY, SqlConnection};
pub use assistant::ApiKeys;
pub use editor::SavedScript;
pub use settings::{Assistant, Settings};
pub use snippets::SnippetStore;

use std::collections::VecDeque;
use std::path::PathBuf;

use egui::{CentralPanel, Panel, RichText, Ui};
use parley_core::SessionConfig;
use parley_sql_core::helper::HelperFiles;
use parley_sql_core::protocol::{self, Connected, Request};
use parley_sql_core::runner::Runner;
use parley_sql_core::text::Risk;

use note::Note;

use parley_ui_egui::theme;

/// Files the SQL window reads and writes through the system's file window, in the app
/// (R-SQL-8, R-SQL-21). `Ok(None)` is the operator cancelling.
pub trait SqlFiles: std::fmt::Debug {
    /// Saves an export where the operator chooses; the path saved to.
    fn export_csv(&self, suggested: &str, bytes: &[u8]) -> Result<Option<String>, String>;
    /// A `.sql` file the operator picks: its path and text.
    fn open_script(&self) -> Result<Option<(String, String)>, String>;
    /// A CSV file the operator picks for an upload (R-SQL-73): its path and text (UTF-8).
    fn open_csv(&self) -> Result<Option<(String, String)>, String>;
    /// Where a download of every row goes (R-SQL-71): a file the operator chooses for
    /// `format`, offered as `suggested`.
    fn download_path(
        &self,
        suggested: &str,
        format: parley_sql_core::transfer::Format,
    ) -> Result<Option<PathBuf>, String>;
    /// Saves `text` to `path`, or where the operator chooses when `path` is `None`; the path.
    fn save_script(
        &self,
        path: Option<&str>,
        suggested: &str,
        text: &str,
    ) -> Result<Option<String>, String>;
    /// Saves `text` as a `.txt` file where the operator chooses, offered as `suggested`; the
    /// path (R-SQL-78).
    fn save_text(&self, suggested: &str, text: &str) -> Result<Option<String>, String>;
    /// Opens `path` in the program the system uses for it (R-SQL-101).
    fn open_file(&self, path: &std::path::Path) -> Result<(), String>;
    /// Opens the folder `path` is in (R-SQL-101).
    fn show_in_folder(&self, path: &std::path::Path) -> Result<(), String>;
}

/// What the SQL window is given by Parley each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Around<'a> {
    /// The runs of this connection, oldest first (R-SQL-89).
    pub history: &'a [parley_sql_core::history::Entry],
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
    /// How Format SQL writes (R-SQL-77).
    pub format: parley_sql_core::text::Style,
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

/// Said when something is asked while a statement or transfer is under way.
const WAIT: &str = "Wait for the statement running to end.";

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
    /// A table's `SELECT *` for a download (R-SQL-71).
    DownloadRun,
    /// More rows from this cursor for a download (R-SQL-71).
    DownloadFetch(u64),
    /// An upload's start, a batch of this many rows, and its end, committing or not (R-SQL-73).
    UploadBegin,
    UploadRows(usize),
    UploadEnd(bool),
}

/// Results, messages or the history under the editor (R-SQL-85), or a System view
/// (R-SQL-78..80).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Results,
    Messages,
    History,
    View(parley_sql_core::services::View),
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

/// A password being typed: its `Debug` does not show it.
#[derive(Default)]
struct Typed(String);

impl std::fmt::Debug for Typed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Typed(…)")
    }
}

/// The answer to the question asked on closing with uncommitted changes (R-SQL-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Commit,
    RollBack,
    Cancel,
}

/// The SQL window for one session.
#[derive(Debug)]
pub struct SqlWindow {
    config: SessionConfig,
    /// The saved connection's database options, sent at sign-on (R-SQL-97..100).
    options: parley_sql_core::options::ConnectOptions,
    /// What separates fields in CSV downloads and exports (R-SQL-100).
    csv_delimiter: parley_sql_core::transfer::CsvDelimiter,
    java: PathBuf,
    files: HelperFiles,
    stage: Stage,
    pub user: String,
    /// What is typed in the password field, until Connect takes it.
    password: Typed,
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
    snippets: snippets::SnippetPanel,
    browser: browser::Browser,
    messages: Vec<Note>,
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
    /// The confirmation's statement's height last frame, for its scroll area.
    confirm_height: f32,
    closing: Closing,
    /// Runs since the last [`SqlWindow::take_ran`], for the history (R-SQL-89).
    ran: Vec<parley_sql_core::history::Entry>,
    /// The History tab's search, filter and selection (R-SQL-90).
    history_list: history::HistoryList,
    /// Removals from the history asked since the last [`SqlWindow::take_history_edits`].
    history_edits: Vec<history::HistoryAction>,
    /// The sign-on, sent once the helper's hello is read; it holds the password until then.
    pending_connect: Option<Request>,
    /// The form for a statement's parameter markers while it is open (R-SQL-40).
    parameters: Option<parameters::ParamForm>,
    /// The values given for each statement's markers, by its text, for this window.
    param_values: std::collections::BTreeMap<String, Vec<parameters::Value>>,
    /// The second connection, for the catalog (R-SQL-33, D21).
    database: database::Database,
    /// The System node's views and viewer windows (R-SQL-78..80).
    system: system::System,
    /// Download every row: the dialog and the download under way (R-SQL-71).
    transfer: download::Transfer,
    /// Upload a CSV file: the dialog and the upload under way (R-SQL-73).
    upload: upload::Upload,
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
    /// The side panel's layout: width, folded, its sections (R-SQL-82, R-SQL-84).
    side: side::SideLayout,
    /// A width to set the side panel to once: the one kept, or the usual one (R-SQL-82).
    side_width: Option<u16>,
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
            options: parley_sql_core::options::ConnectOptions::default(),
            csv_delimiter: parley_sql_core::transfer::CsvDelimiter::default(),
            java,
            files,
            stage: Stage::SignIn,
            user: user.to_owned(),
            password: Typed::default(),
            runner: None,
            connected: None,
            scripts: editor::Scripts::restore(scripts),
            queue: VecDeque::new(),
            waiting: None,
            run_started: None,
            results: results::Results::reading(config.code_page),
            tab: Tab::Results,
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
            confirm_height: 0.0,
            closing: Closing::Open,
            ran: Vec::new(),
            history_list: history::HistoryList::default(),
            history_edits: Vec::new(),
            pending_connect: None,
            database: database::Database::default(),
            system: system::System::default(),
            transfer: download::Transfer::default(),
            upload: upload::Upload::default(),
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
            side: side::SideLayout::default(),
            side_width: None,
        }
    }

    /// Drawn with [`SqlWindow::show_in`] in Parley SQL's main window (R-SQL-45): its Close
    /// button says Disconnect, and the main window asks before closing (R-SQL-5).
    #[must_use]
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
    #[must_use]
    pub const fn with_header(mut self, header: grid::HeaderMode) -> Self {
        self.header = header;
        self
    }

    /// CSV downloads and exports separate fields with `delimiter` (R-SQL-100).
    #[must_use]
    pub const fn with_csv(mut self, delimiter: parley_sql_core::transfer::CsvDelimiter) -> Self {
        self.csv_delimiter = delimiter;
        self.results.csv_delimiter = delimiter;
        self
    }

    /// Signs on with the saved connection's `options`, and starts Autocommit as they say
    /// (R-SQL-97..100).
    #[must_use]
    pub fn with_options(mut self, options: parley_sql_core::options::ConnectOptions) -> Self {
        self.autocommit = options.autocommit;
        self.options = options;
        self
    }

    /// The side panel laid out as `side` and the library groups `groups` of this connection
    /// (R-SQL-82, R-SQL-83).
    #[must_use]
    pub fn with_side(mut self, side: side::SideLayout, groups: Vec<browser::LibraryGroup>) -> Self {
        self.browser =
            browser::Browser::with_groups(groups, side.library_list_open, side.system_open);
        self.side = side;
        self.side_width = Some(side.width);
        self
    }

    /// Shows the SQL assistant while it is on (R-ADD-3), asking `config`'s provider with a key
    /// from the environment or `keys` (R-SQL-48).
    pub fn set_assistant(
        &mut self,
        on: bool,
        config: &parley_sql_core::assistant::AssistantConfig,
        keys: Option<&std::sync::Arc<dyn assistant::ApiKeys>>,
    ) {
        self.assistant.on = on;
        if self.assistant.config != *config {
            self.assistant.config.clone_from(config);
        }
        self.assistant.keys = keys.cloned();
    }

    /// The library groups now, to keep in the settings (R-SQL-83).
    pub fn library_groups(&self) -> &[browser::LibraryGroup] {
        self.browser.library_groups()
    }

    /// Fetches `rows` at a time and stops a statement after `timeout` seconds (0: never).
    #[must_use]
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

    /// The runs since last asked, for the history (R-SQL-89).
    pub fn take_ran(&mut self) -> Vec<parley_sql_core::history::Entry> {
        std::mem::take(&mut self.ran)
    }

    /// Remove from history and Clear history, asked since last taken (R-SQL-90, R-SQL-91).
    pub fn take_history_edits(&mut self) -> Vec<history::HistoryAction> {
        std::mem::take(&mut self.history_edits)
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

    /// Whether closing was asked and is not yet answered or cancelled (R-SQL-5).
    pub(crate) fn is_closing(&self) -> bool {
        match self.closing {
            Closing::Open => false,
            Closing::Asking | Closing::AfterCommit | Closing::Closed => true,
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
                self.side_panel(ui, around);
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
        self.browser.dialogs(ui.ctx());
        if let Some(chosen) = self.browser.take_chosen() {
            self.generate_chosen(chosen, around.files);
        }
        self.system.viewers(ui.ctx(), around.files);
        self.confirm_dialog(ui.ctx());
        self.download_dialog(ui.ctx(), around.files);
        self.upload_dialog(ui.ctx(), around.files);
        self.parameters_dialog(ui.ctx());
        self.closing_dialog(ui.ctx());
    }

    /// Ends the helper: the connection closes with it.
    fn finish(&mut self) {
        // A download that did not end leaves no part of its file (R-SQL-71).
        self.cancel_download();
        self.end_download(None);
        if let Some(mut runner) = self.runner.take() {
            runner.send(Request::Close);
        }
        self.database.close();
    }
}

/// "Show in the editor" beside an error whose place is marked (R-SQL-75); true when clicked.
fn show_link(ui: &mut Ui) -> bool {
    ui.link(RichText::new("Show in the editor").small())
        .on_hover_text("Puts the cursor where the host says the statement went wrong")
        .clicked()
}

/// What Parley SQL's UI tests share; Parley's own (fonts, fake sessions) are in
/// `parley_ui_egui::test_support`.
#[cfg(test)]
pub(crate) mod test_support {
    pub use parley_ui_egui::test_support::install_fonts;

    /// The last button labelled `button` (a dialog's, drawn over the window) stays where it is
    /// from frame to frame (D12).
    pub fn keeps_its_place<S>(harness: &mut egui_kittest::Harness<'_, S>, button: &str) {
        use egui_kittest::kittest::Queryable;
        let place = |h: &egui_kittest::Harness<'_, S>| {
            h.get_all_by_label(button)
                .last()
                .unwrap_or_else(|| panic!("a button {button}"))
                .rect()
        };
        harness.run_steps(2);
        let before = place(harness);
        harness.run_steps(5);
        assert_eq!(place(harness), before, "D12: {button} moved");
    }

    /// A keychain in memory, for tests.
    #[derive(Default)]
    pub struct FakeKeys(
        pub  std::sync::Mutex<
            std::collections::HashMap<parley_sql_core::assistant::Provider, String>,
        >,
    );

    impl std::fmt::Debug for FakeKeys {
        /// Like a real keychain's: the keys are not shown.
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("FakeKeys")
        }
    }

    impl crate::ApiKeys for FakeKeys {
        fn get(
            &self,
            provider: parley_sql_core::assistant::Provider,
        ) -> Result<Option<String>, String> {
            Ok(self
                .0
                .lock()
                .map_err(|e| e.to_string())?
                .get(&provider)
                .cloned())
        }
        fn set(
            &self,
            provider: parley_sql_core::assistant::Provider,
            key: &str,
        ) -> Result<(), String> {
            self.0
                .lock()
                .map_err(|e| e.to_string())?
                .insert(provider, key.to_owned());
            Ok(())
        }
        fn delete(&self, provider: parley_sql_core::assistant::Provider) -> Result<(), String> {
            self.0.lock().map_err(|e| e.to_string())?.remove(&provider);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::grid::ResultView;
    use parley_sql_core::protocol::Answer;
    use parley_sql_core::runner::Event;
    use parley_sql_core::text;

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
        assert_eq!(statements(w.take_ran()), ["SELECT NAME FROM T"]);
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
        assert!(
            w.messages
                .iter()
                .any(|m| m.text().contains("3 rows changed"))
        );
        w.request_close();
        assert_eq!(w.closing, Closing::Asking);
    }

    #[test]
    fn the_closing_question_keeps_its_buttons_in_place_and_cancel_keeps_the_window_r_sql_5() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        w.uncommitted = true;
        w.request_close();
        let mut harness = drawn(w, Vec::new());
        harness.get_by_label("Changes are not committed");
        test_support::keeps_its_place(&mut harness, "Roll back");
        harness
            .get_all_by_label("Cancel")
            .last()
            .expect("the question's Cancel")
            .click();
        harness.run();
        assert_eq!(harness.state().closing, Closing::Open);
        assert!(
            harness.state().uncommitted,
            "nothing committed or rolled back"
        );
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
            statements(w.take_ran()),
            ["SELECT * FROM NOPE"],
            "failed statements are in the history too"
        );
    }

    #[test]
    fn run_all_and_run_send_a_procedure_whole_r_sql_92() {
        let procedure = "CREATE OR REPLACE PROCEDURE NODEJT400.TÆL (IN N INT) \
                         BEGIN DECLARE I INT DEFAULT 0; \
                         WHILE I < N DO SET I = I + 1; END WHILE; END";
        // The DELETE first holds Run all at its question, with the rest still queued.
        let script = format!("DELETE FROM LOG;\n{procedure};\nCALL NODEJT400.TÆL(3)");
        let mut w = window();
        w.scripts.set_text(&script, 0);
        w.run_all();
        assert!(w.confirm.is_some(), "the DELETE asks first");
        assert_eq!(w.queue, [procedure, "CALL NODEJT400.TÆL(3)"]);

        let mut w = window();
        let inside = script.find("SET I").expect("in the body");
        w.scripts.set_text(&script, inside);
        assert_eq!(
            w.scripts.current_statement().as_deref(),
            Some(procedure),
            "the cursor in the body runs the whole procedure"
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
        let config = parley_sql_core::assistant::AssistantConfig::default();
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
        "TopSecret9".clone_into(&mut w.password.0);
        w.connect(&egui::Context::default());
        assert_eq!(w.password.0, "", "not kept in the field");
        assert_eq!(w.stage, Stage::SignIn);
        let line = w.error_line.as_deref().expect("why");
        assert!(!line.contains("TopSecret9"), "{line}");
    }

    #[test]
    fn the_window_s_debug_does_not_show_the_password_typed_r_sql_6() {
        let mut w = window();
        "TopSecret9æ".clone_into(&mut w.password.0);
        let shown = format!("{w:?}");
        assert!(!shown.contains("TopSecret9"), "{shown}");
        assert!(shown.contains("password: Typed(…)"));
    }

    #[test]
    fn a_long_statement_to_confirm_scrolls_and_its_buttons_stay_in_the_window_r_sql_9() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        let long = format!("UPDATE T SET A = A\n{}", vec!["|| 'æøå'"; 200].join("\n"));
        w.confirm = text::risk(&long).map(|risk| (long.clone(), risk));
        assert!(w.confirm.is_some(), "a delete is asked about");
        let mut harness = drawn(w, Vec::new());
        harness.run_steps(3);
        let confirm_run = |h: &egui_kittest::Harness<'static, SqlWindow>| {
            h.get_all_by_label("Run")
                .last()
                .expect("the dialog's Run")
                .rect()
        };
        let run = confirm_run(&harness);
        assert!(run.bottom() <= 760.0, "{run:?} is in the window");
        harness.run_steps(5);
        assert_eq!(confirm_run(&harness), run, "D12");
    }

    #[test]
    fn character_positions_become_byte_offsets_r_sql_7() {
        assert_eq!(editor::byte_at("æøå;", 3), 6);
        assert_eq!(editor::byte_at("abc", 9), 3);
    }

    /// The statements of `ran`, for tests of what goes into the history.
    pub(super) fn statements(ran: Vec<parley_sql_core::history::Entry>) -> Vec<String> {
        ran.into_iter().map(|e| e.statement).collect()
    }

    /// The window drawn alone, as in a test of the main window (embedded: kittest has one),
    /// with `history` as statements kept before Phase 19, without time or outcome.
    pub(super) fn drawn(
        w: SqlWindow,
        history: Vec<String>,
    ) -> egui_kittest::Harness<'static, SqlWindow> {
        let history = history
            .into_iter()
            .map(|statement| parley_sql_core::history::Entry {
                statement,
                at: None,
                took_ms: None,
                outcome: None,
            })
            .collect();
        drawn_with_history(w, history)
    }

    /// The window drawn alone with the runs `history` (R-SQL-89).
    pub(super) fn drawn_with_history(
        w: SqlWindow,
        history: Vec<parley_sql_core::history::Entry>,
    ) -> egui_kittest::Harness<'static, SqlWindow> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(1100.0, 760.0))
            .with_max_steps(16)
            .build_ui_state(
                move |ui, w: &mut SqlWindow| {
                    parley_ui_egui::test_support::install_fonts(ui);
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
            "Fold Explorer",
            "Fold Snippets",
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
                    parley_ui_egui::test_support::install_fonts(ui);
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

    /// R-SQL-82: the side panel folds to a strip and opens again, by its buttons and by
    /// Ctrl+B, and the editor gets the room.
    #[test]
    fn the_side_panel_folds_away_and_opens_again_r_sql_82() {
        use egui_kittest::kittest::Queryable;

        let mut harness = drawn_with_results(1100.0, layout::ResultsPlace::Below);
        let wide = harness
            .get_by_label("Editor and results divider")
            .rect()
            .width();
        harness.get_by_label("Hide the side panel").click();
        harness.run();
        assert!(harness.query_by_label("Fold Explorer").is_none());
        assert!(harness.state().side_layout().folded, "kept in the settings");
        let wider = harness
            .get_by_label("Editor and results divider")
            .rect()
            .width();
        assert!(wider > wide + 150.0, "{wide} then {wider}");
        harness.get_by_label("Show Explorer and Snippets").click();
        harness.run();
        harness.get_by_label("Fold Explorer");
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        harness.run();
        assert!(harness.query_by_label("Fold Explorer").is_none(), "Ctrl+B");
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        harness.run();
        harness.get_by_label("Hide the side panel");
    }

    /// R-SQL-84: each section folds on its own, and the other gets the room.
    #[test]
    fn libraries_and_snippets_fold_on_their_own_r_sql_84() {
        use egui_kittest::kittest::Queryable;

        let mut harness = drawn_with_results(1100.0, layout::ResultsPlace::Below);
        let snippets = harness.get_by_label("Fold Snippets").rect().top();
        harness.get_by_label("Fold Explorer").click();
        harness.run();
        let higher = harness.get_by_label("Fold Snippets").rect().top();
        assert!(higher < snippets - 100.0, "{snippets} then {higher}");
        assert!(!harness.state().side_layout().libraries_open);
        harness.get_by_label("Open Explorer").click();
        harness.run();
        harness.get_by_label("Fold Snippets").click();
        harness.run();
        harness.get_by_label("Open Snippets");
        harness.get_by_label("Fold Explorer");
    }

    /// R-SQL-85: History is a tab beside Results and Messages; R-SQL-91: a click selects a
    /// statement, Enter adds it to the script.
    #[test]
    fn the_history_is_a_tab_under_the_editor_r_sql_85_r_sql_91() {
        use egui_kittest::kittest::Queryable;

        let statement = "SELECT 'æøå' FROM T";
        let mut harness = drawn(window(), vec![statement.to_owned()]);
        assert!(harness.query_by_label(statement).is_none());
        harness.get_by_label("History").click();
        harness.run();
        harness.get_by_label(statement).click();
        harness.run();
        assert!(
            !harness.state().scripts.text().contains(statement),
            "selected only"
        );
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert!(harness.state().scripts.text().contains(statement));
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
    fn the_snippets_section_lists_the_starters_and_runs_one_r_sql_25_r_sql_84() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        running(&mut w, "VALUES 1");
        // Searched for, so it is in view among the starters (R-SQL-25).
        w.snippets.query = "system status".to_owned();
        let mut harness = drawn(w, Vec::new());
        // The spinner keeps repainting while a statement runs.
        harness.run_steps(3);
        // R-SQL-88: Run shows only while the pointer is on the snippet's line.
        assert!(harness.query_by_label("Run System status").is_none());
        harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "System status")
            .hover();
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

    /// R-SQL-88: the snippets are groups that fold; a snippet's right-click menu inserts or
    /// runs it, and a starter, which cannot be changed, has no Edit or Delete.
    #[test]
    fn a_snippet_s_menu_inserts_it_and_a_starter_cannot_be_edited_r_sql_88() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        w.snippets.query = "system status".to_owned();
        let mut harness = drawn(w, Vec::new());
        harness.run_steps(3);
        harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "System status")
            .click_secondary();
        harness.run_steps(3);
        assert!(harness.query_by_label("Edit…").is_none(), "a starter");
        assert!(harness.query_by_label("Delete…").is_none(), "a starter");
        harness.get_by_label("Insert").click();
        harness.run_steps(3);
        assert!(
            harness
                .state()
                .scripts
                .text()
                .contains("SYSTEM_STATUS_INFO"),
            "inserted at the cursor"
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
        assert_eq!(statements(w.take_ran()), ["CL: ADDLIBLE MYLIB"]);
        assert_eq!(
            w.messages.last().map(Note::text),
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

    #[test]
    fn shift_enter_in_the_find_bar_goes_to_the_match_before_r_sql_23() {
        use egui_kittest::kittest::Queryable;
        let text = "VALUES 'å'; VALUES 'æ'; VALUES 'ø'";
        let second = text.find("VALUES 'æ'").expect("there");
        let third = text.rfind("VALUES").expect("there");
        let mut w = window();
        w.scripts.set_text(text, third);
        let mut harness = drawn(w, Vec::new());
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
        harness.run();
        harness.get_by_label("Match case");
        // The query field has the focus once the bar opens.
        harness.event(egui::Event::Text("values".to_owned()));
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
        harness.run();
        assert_eq!(harness.state().scripts.selection(), (second, second + 6));
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
                    parley_ui_egui::test_support::install_fonts(ui);
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
    fn the_editor_s_keys_wait_while_save_the_script_is_asked_r_sql_20() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        w.scripts.set_text("VALUES 'æøå'", 0);
        let mut harness = drawn(w, Vec::new());
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
        harness.run();
        harness.get_by_label("Save the script?");
        test_support::keeps_its_place(&mut harness, "Don't save");
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::T);
        harness.run();
        assert!(
            harness.query_by_label("Script 2").is_none(),
            "no tab added under the question"
        );
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(
            harness.query_by_label("Save the script?").is_none(),
            "Esc closes it"
        );
        assert_eq!(harness.state().scripts().1.len(), 1);
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
