//! Download every row (R-SQL-71, R-SQL-72, D31): the dialog, then the rows written to the file
//! as they come, from a result tab's open cursor or from a table read whole; as `INSERT`
//! statements, one table's or, after its SQL, a whole library's (R-SQL-95, R-SQL-96).

use std::collections::{BTreeSet, VecDeque};

use egui::{Context, Ui};
use parley_sql_core::catalog;
use parley_sql_core::generate::DataTable;
use parley_sql_core::protocol::{Column, MoreRows, Outcome, Request, Row};
use parley_sql_core::runner::Event;
use parley_sql_core::transfer::{self, Format};

use super::browser::{self, Goal};
use super::grid::{HeaderMode, header_text};
use super::{Note, SqlFiles, SqlWindow, WAIT, Waiting};
use parley_ui_egui::{dialog, theme};

/// What is downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The result tab shown when the dialog opened.
    Tab,
    /// A table or view of the schema browser, read whole.
    Table { library: String, name: String },
}

/// The dialog's choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    source: Source,
    format: Format,
    /// IBM i column headings as the first row, instead of the field names (R-SQL-29).
    headings: bool,
}

/// A download under way.
#[derive(Debug)]
pub struct Downloading {
    /// `None` until a table's first rows came, which give the columns.
    writer: Option<transfer::Download>,
    /// For a table: the file, its format and headings, until the writer is made.
    pending: Option<(std::path::PathBuf, Format, bool, String)>,
    /// The cursor the rest comes from, while there is more.
    cursor: Option<u64>,
    cancelled: bool,
    /// A backup's tables, read one after the other into its SQL file (R-SQL-95, R-SQL-96).
    tables: Option<Tables>,
}

/// The tables of a backup.
#[derive(Debug)]
pub struct Tables {
    library: String,
    left: VecDeque<DataTable>,
    /// The table being read, as written in SQL, and whether its rows override.
    current: Option<(String, bool)>,
    /// What the file ends with after the rows.
    after: String,
    read: usize,
    whole: bool,
}

/// The download dialog and the download under way.
#[derive(Debug, Default)]
pub struct Transfer {
    dialog: Option<Choice>,
    pub running: Option<Downloading>,
    /// The file the last download finished, for Open and Show in folder (R-SQL-101).
    pub done: Option<std::path::PathBuf>,
}

impl Transfer {
    /// Opens the dialog for `source`.
    pub fn ask(&mut self, source: Source) {
        self.dialog = Some(Choice {
            source,
            format: Format::Csv,
            headings: false,
        });
    }

    /// Rows written so far, while a download runs.
    pub fn progress(&self) -> Option<u64> {
        self.running
            .as_ref()
            .map(|d| d.writer.as_ref().map_or(0, transfer::Download::rows))
    }
}

/// The names of the header row: the field names, or the IBM i column headings.
fn names(columns: &[Column], headings: bool) -> Vec<String> {
    let mode = if headings {
        HeaderMode::Headings
    } else {
        HeaderMode::Names
    };
    columns.iter().map(|c| header_text(c, mode)).collect()
}

impl SqlWindow {
    /// The download dialog while open; Download asks the file window for the file and starts.
    pub(super) fn download_dialog(&mut self, ctx: &Context, files: Option<&dyn SqlFiles>) {
        let blocked = self.busy().then_some(WAIT);
        let Some(choice) = self.transfer.dialog.as_mut() else {
            return;
        };
        let (close, (cancel, start)) =
            dialog::dialog(ctx, "sql-download", "Download", 440.0, |ui| {
                choose(ui, choice, blocked)
            });
        if close || cancel {
            self.transfer.dialog = None;
        } else if start {
            let choice = self.transfer.dialog.take().expect("the dialog was open");
            self.start_download(choice, files);
        }
    }

    fn start_download(&mut self, choice: Choice, files: Option<&dyn SqlFiles>) {
        let Some(files) = files else {
            return;
        };
        // One request at a time: started now, it would take the place of the one waited for.
        if self.busy() {
            return self.fail(WAIT.to_owned());
        }
        let suggested = match &choice.source {
            Source::Tab => self
                .results
                .shown_columns()
                .map_or_else(|| "query".to_owned(), transfer::suggested_name),
            Source::Table { name, .. } => name.clone(),
        };
        let file = format!("{suggested}.{}", choice.format.extension());
        let path = match files.download_path(&file, choice.format) {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(e) => return self.fail(e),
        };
        // A failure said before is not this download's.
        self.error_line = None;
        match choice.source {
            Source::Tab => {
                let Some((columns, rows, cursor)) = self.results.take_for_download() else {
                    return;
                };
                let sheet = transfer::suggested_name(&columns);
                let names = names(&columns, choice.headings);
                match transfer::Download::create(
                    &path,
                    choice.format,
                    &columns,
                    &names,
                    &sheet,
                    self.csv_delimiter,
                ) {
                    Ok(writer) => {
                        self.transfer.done = None;
                        self.transfer.running = Some(Downloading {
                            writer: Some(writer),
                            pending: None,
                            cursor,
                            cancelled: false,
                            tables: None,
                        });
                        self.download_rows(&rows, cursor.is_some());
                    }
                    Err(e) => self.fail(e.to_string()),
                }
            }
            // As INSERT statements: the catalog says first whether its rows override (R-SQL-95).
            Source::Table { library, name } if choice.format == Format::Sql => {
                let goal = Goal::Backup {
                    path,
                    only: Some(name),
                };
                self.browser
                    .generate(&library, BTreeSet::new(), goal, &mut self.database);
            }
            Source::Table { library, name } => {
                self.transfer.done = None;
                self.transfer.running = Some(Downloading {
                    writer: None,
                    pending: Some((path, choice.format, choice.headings, name.clone())),
                    cursor: None,
                    cancelled: false,
                    tables: None,
                });
                let run = Request::Run {
                    sql: format!("SELECT * FROM {}", catalog::qualified(&library, &name)),
                    max_rows: self.rows,
                    timeout: self.timeout,
                    params: Vec::new(),
                };
                self.send(run, Waiting::DownloadRun);
            }
        }
    }

    /// Writes `rows`, then asks for more while there are, or finishes.
    fn download_rows(&mut self, rows: &[Row], more: bool) {
        let Some(running) = self.transfer.running.as_mut() else {
            return;
        };
        let written = running.writer.as_mut().map(|w| w.write(rows));
        if let Some(Err(e)) = written {
            return self.end_download(Some(e.to_string()));
        }
        if !more {
            running.cursor = None;
        }
        match running.cursor {
            Some(cursor) if !running.cancelled => {
                let fetch = Request::Fetch {
                    cursor,
                    max_rows: self.rows,
                };
                self.send(fetch, Waiting::DownloadFetch(cursor));
            }
            Some(_) | None => {
                let more_tables = !running.cancelled
                    && running.tables.as_ref().is_some_and(|t| !t.left.is_empty());
                if more_tables {
                    self.next_table();
                } else {
                    self.end_download(None);
                }
            }
        }
    }

    /// Generate SQL… or Back up as SQL… chosen on a library (R-SQL-93, R-SQL-96): a backup
    /// asks for its file first.
    pub(super) fn generate_chosen(
        &mut self,
        chosen: browser::Chosen,
        files: Option<&dyn SqlFiles>,
    ) {
        let goal = if chosen.backup {
            let Some(files) = files else {
                return;
            };
            if self.busy() {
                return self.fail(WAIT.to_owned());
            }
            let file = format!("{}.{}", chosen.library, Format::Sql.extension());
            match files.download_path(&file, Format::Sql) {
                Ok(Some(path)) => Goal::Backup { path, only: None },
                Ok(None) => return,
                Err(e) => return self.fail(e),
            }
        } else {
            Goal::Script
        };
        self.browser
            .generate(&chosen.library, chosen.kinds, goal, &mut self.database);
    }

    /// A backup's SQL came from the catalog: the file starts with it, then each table's rows
    /// are read into it (R-SQL-95, R-SQL-96).
    pub(super) fn start_backup(&mut self, backup: browser::Backup) {
        let not = if backup.whole {
            "Not backed up"
        } else {
            "Not downloaded"
        };
        // The SQL was read on the catalog job meanwhile; a statement may have started.
        if self.busy() {
            return self.fail(format!("{not}: {WAIT}"));
        }
        let writer = match transfer::Download::script(&backup.path, &backup.before) {
            Ok(writer) => writer,
            Err(e) => return self.fail(format!("{not}: {e}")),
        };
        self.error_line = None;
        self.transfer.done = None;
        self.transfer.running = Some(Downloading {
            writer: Some(writer),
            pending: None,
            cursor: None,
            cancelled: false,
            tables: Some(Tables {
                library: backup.library,
                left: backup.tables.into(),
                current: None,
                after: backup.after,
                read: 0,
                whole: backup.whole,
            }),
        });
        self.next_table();
    }

    /// Reads the backup's next table, or ends it when there is none.
    fn next_table(&mut self) {
        let Some(tables) = self
            .transfer
            .running
            .as_mut()
            .and_then(|r| r.tables.as_mut())
        else {
            return;
        };
        let Some(table) = tables.left.pop_front() else {
            return self.end_download(None);
        };
        let name = catalog::qualified(&tables.library, &table.name);
        tables.current = Some((name.clone(), table.overriding));
        let run = Request::Run {
            sql: format!("SELECT * FROM {name}"),
            max_rows: self.rows,
            timeout: self.timeout,
            params: Vec::new(),
        };
        self.send(run, Waiting::DownloadRun);
    }

    /// A backup's table answered: its rows go into the file after a comment naming it.
    fn table_rows(&mut self, columns: &[Column], rows: &[Row], more: bool, cursor: u64) {
        let Some(running) = self.transfer.running.as_mut() else {
            return;
        };
        let (Some(writer), Some(tables)) = (running.writer.as_mut(), running.tables.as_mut())
        else {
            return;
        };
        let Some((name, overriding)) = tables.current.take() else {
            return;
        };
        tables.read += 1;
        if tables.whole
            && let Err(e) = writer.text(&format!("\n-- The rows of {name}\n"))
        {
            return self.end_download(Some(e.to_string()));
        }
        writer.table(&name, columns, overriding);
        running.cursor = more.then_some(cursor);
        self.download_rows(rows, more);
    }

    /// The answer to a table's `SELECT *`: the writer is made from its columns.
    pub(super) fn on_download_run(&mut self, event: Event) {
        let outcome = event
            .result
            .and_then(parley_sql_core::protocol::Answer::into_result::<Outcome>);
        let Some(running) = self.transfer.running.as_mut() else {
            return;
        };
        match outcome {
            Ok(Outcome::Rows {
                cursor,
                columns,
                rows,
                more,
            }) => {
                let Some((path, format, headings, sheet)) = running.pending.take() else {
                    return self.table_rows(&columns, &rows, more, cursor);
                };
                running.cursor = more.then_some(cursor);
                let names = names(&columns, headings);
                match transfer::Download::create(
                    &path,
                    format,
                    &columns,
                    &names,
                    &sheet,
                    self.csv_delimiter,
                ) {
                    Ok(writer) => {
                        running.writer = Some(writer);
                        self.download_rows(&rows, more);
                    }
                    Err(e) => self.end_download(Some(e.to_string())),
                }
            }
            Ok(Outcome::Count { .. }) => {
                self.end_download(Some("The table gave no rows to download.".to_owned()));
            }
            Err(e) => self.end_download(Some(e.to_string())),
        }
    }

    /// More rows for the download.
    pub(super) fn on_download_fetch(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_sql_core::protocol::Answer::into_result::<MoreRows>)
        {
            Ok(more) => self.download_rows(&more.rows, more.more),
            Err(e) => self.end_download(Some(e.to_string())),
        }
    }

    /// Cancel: the fetch running stops, and the file goes when its answer comes.
    pub(super) fn cancel_download(&mut self) {
        if let Some(running) = self.transfer.running.as_mut() {
            running.cancelled = true;
        }
    }

    /// Ends the download: finished, or (cancelled or `error`) removed. An open cursor is closed.
    pub(super) fn end_download(&mut self, error: Option<String>) {
        let Some(running) = self.transfer.running.take() else {
            return;
        };
        if let Some(cursor) = running.cursor
            && let Some(runner) = &mut self.runner
        {
            // The rest is not wanted; the helper frees the cursor and nothing waits for it.
            runner.send(Request::CloseCursor { cursor });
        }
        let Some(writer) = running.writer else {
            if running.cancelled {
                self.messages
                    .push(Note::Info("Download stopped.".to_owned()));
            } else if let Some(e) = error {
                self.fail(format!("Not downloaded: {e}"));
            }
            return;
        };
        let path = writer.path().display().to_string();
        // Cancel stops the fetch running, which then answers with an error: stopped, not failed.
        if running.cancelled {
            writer.abandon();
            self.messages
                .push(Note::Info(format!("Download stopped; {path} removed.")));
            return;
        }
        let whole = running.tables.as_ref().is_some_and(|t| t.whole);
        let not = if whole {
            "Not backed up"
        } else {
            "Not downloaded"
        };
        let mut writer = writer;
        let after = running.tables.as_ref().map_or("", |t| t.after.as_str());
        let error = error.or_else(|| writer.text(after).err().map(|e| e.to_string()));
        if let Some(e) = error {
            writer.abandon();
            return self.fail(format!("{not}: {e}"));
        }
        let finished = writer.finish();
        if finished.is_ok() {
            self.transfer.done = Some(std::path::PathBuf::from(&path));
        }
        match finished {
            Ok(rows) if whole => {
                let tables = running.tables.as_ref().map_or(0, |t| t.read);
                self.messages.push(Note::Info(format!(
                    "Backed up {tables} tables, {rows} rows, to {path}."
                )));
            }
            Ok(rows) => self
                .messages
                .push(Note::Info(format!("Downloaded {rows} rows to {path}."))),
            Err(e) => self.fail(format!("{not}: {e}")),
        }
        self.tab = super::Tab::Messages;
    }
}

/// The dialog's content: the format and the header row; (Cancel, Download).
fn choose(ui: &mut Ui, choice: &mut Choice, blocked: Option<&str>) -> (bool, bool) {
    let what = match &choice.source {
        Source::Tab => "Every row of the result shown, not only those fetched.".to_owned(),
        Source::Table { library, name } => {
            format!("Every row of {}.", catalog::qualified(library, name))
        }
    };
    ui.label(what);
    theme::hint(
        ui,
        "In the host's order: the grid's sort and filter do not apply. The rows are written as they come.",
    );
    ui.add_space(10.0);
    theme::section_label(ui, "Format");
    theme::segmented(
        ui,
        &mut choice.format,
        &[
            (Format::Csv, "CSV"),
            (Format::Excel, "Excel"),
            (Format::Sql, "SQL INSERT"),
        ],
    );
    ui.add_space(8.0);
    if choice.format == Format::Sql {
        theme::hint(
            ui,
            "One INSERT statement a row, into the table the rows come from: a backup that can be run.",
        );
    } else {
        theme::switch_row(
            ui,
            "IBM i column headings as the first row",
            &mut choice.headings,
        );
    }
    if let Some(why) = blocked {
        theme::warning(ui, why);
    }
    dialog::actions(
        ui,
        theme::primary_button(ui, "Download…"),
        blocked.is_none(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::ResultView;
    use crate::tests::{drawn, window};
    use parley_sql_core::protocol::Answer;
    use std::path::PathBuf;

    /// A file window that picks `path` without asking.
    #[derive(Debug)]
    struct PicksFile(PathBuf);

    impl SqlFiles for PicksFile {
        fn open_file(&self, _: &std::path::Path) -> Result<(), String> {
            Ok(())
        }
        fn show_in_folder(&self, _: &std::path::Path) -> Result<(), String> {
            Ok(())
        }
        fn export_csv(&self, _: &str, _: &[u8]) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn open_script(&self) -> Result<Option<(String, String)>, String> {
            Ok(None)
        }
        fn open_csv(&self) -> Result<Option<(String, String)>, String> {
            Ok(None)
        }
        fn download_path(&self, _: &str, _: Format) -> Result<Option<PathBuf>, String> {
            Ok(Some(self.0.clone()))
        }
        fn save_script(&self, _: Option<&str>, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn save_text(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
    }

    /// A file in a folder of its own, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("parley-download-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a scratch folder");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // A test's scratch folder; leaving it behind harms nothing.
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn column_json(name: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name, "type": "VARCHAR", "length": 10, "scale": 0,
            "nullable": true, "numeric": false, "heading": format!("{name} heading"),
        })
    }

    fn column(name: &str) -> Column {
        serde_json::from_value(column_json(name)).expect("a column")
    }

    fn rows(values: &[&str]) -> Vec<Row> {
        values.iter().map(|v| vec![Some((*v).to_owned())]).collect()
    }

    /// The helper's answer `ok`, as the runner hands it over.
    fn answer(ok: serde_json::Value) -> Event {
        Event {
            tag: 0,
            result: Ok(Answer {
                id: Some(1),
                outcome: Ok(ok),
            }),
        }
    }

    fn choice(source: Source, headings: bool) -> Choice {
        Choice {
            source,
            format: Format::Csv,
            headings,
        }
    }

    #[test]
    fn a_tab_download_writes_the_rows_fetched_then_the_rest_of_the_cursor_r_sql_71() {
        let scratch = Scratch::new("tab");
        let path = scratch.0.join("t.csv");
        let mut w = window();
        w.results.add(
            "SELECT NAVN FROM T",
            ResultView::new(vec![column("NAVN")], rows(&["æøå", "b"]), true, 7),
        );
        w.start_download(choice(Source::Tab, false), Some(&PicksFile(path.clone())));
        assert_eq!(
            w.transfer.progress(),
            Some(2),
            "the rows fetched, written at once"
        );
        assert_eq!(
            w.results.take_for_download().and_then(|(_, _, c)| c),
            None,
            "the download took the cursor from the grid"
        );
        w.on_download_fetch(answer(serde_json::json!({"rows": [["c"]], "more": false})));
        assert!(w.transfer.running.is_none(), "done");
        let text = std::fs::read_to_string(&path).expect("the file");
        assert_eq!(text, "\u{FEFF}NAVN\r\næøå\r\nb\r\nc\r\n");
        assert!(
            w.messages
                .iter()
                .any(|m| m.text().starts_with("Downloaded 3 rows"))
        );
    }

    /// A file window that records what it was asked to open (R-SQL-101).
    #[derive(Debug, Default)]
    struct Opens(std::sync::Mutex<Vec<(&'static str, PathBuf)>>);

    impl SqlFiles for Opens {
        fn export_csv(&self, _: &str, _: &[u8]) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn open_script(&self) -> Result<Option<(String, String)>, String> {
            Ok(None)
        }
        fn open_csv(&self) -> Result<Option<(String, String)>, String> {
            Ok(None)
        }
        fn download_path(&self, _: &str, _: Format) -> Result<Option<PathBuf>, String> {
            Ok(None)
        }
        fn save_script(&self, _: Option<&str>, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn save_text(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn open_file(&self, path: &std::path::Path) -> Result<(), String> {
            self.0
                .lock()
                .expect("not poisoned")
                .push(("open", path.to_owned()));
            Ok(())
        }
        fn show_in_folder(&self, path: &std::path::Path) -> Result<(), String> {
            self.0
                .lock()
                .expect("not poisoned")
                .push(("folder", path.to_owned()));
            Ok(())
        }
    }

    #[test]
    fn a_finished_download_offers_open_and_show_in_folder_r_sql_101() {
        use egui_kittest::kittest::Queryable;
        let scratch = Scratch::new("open");
        let path = scratch.0.join("æøå.csv");
        let mut w = window();
        w.results.add(
            "SELECT NAVN FROM T",
            ResultView::new(vec![column("NAVN")], rows(&["a"]), false, 7),
        );
        w.start_download(choice(Source::Tab, false), Some(&PicksFile(path.clone())));
        assert_eq!(w.transfer.done.as_deref(), Some(path.as_path()));
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 400.0))
            .build_ui_state(
                |ui, (w, files): &mut (crate::SqlWindow, Opens)| {
                    parley_ui_egui::test_support::install_fonts(ui);
                    w.messages(ui, Some(&*files));
                },
                (w, Opens::default()),
            );
        harness.run();
        harness.get_by_label("Open").click();
        harness.run();
        harness.get_by_label("Show in folder").click();
        harness.run();
        let asked = harness.state().1.0.lock().expect("not poisoned").clone();
        assert_eq!(asked, [("open", path.clone()), ("folder", path)]);
        harness.get_by_label("Close download").click();
        harness.run();
        assert!(harness.state().0.transfer.done.is_none());
        assert!(harness.query_by_label("Open").is_none());
    }

    #[test]
    fn cancel_stops_the_download_and_removes_the_file_r_sql_71() {
        let scratch = Scratch::new("cancel");
        let path = scratch.0.join("t.csv");
        let mut w = window();
        w.results.add(
            "SELECT NAVN FROM T",
            ResultView::new(vec![column("NAVN")], rows(&["a"]), true, 7),
        );
        w.start_download(choice(Source::Tab, false), Some(&PicksFile(path.clone())));
        assert!(path.exists(), "being written");
        w.cancel();
        // The helper answers the stopped fetch with an error.
        w.on_download_fetch(Event {
            tag: 0,
            result: Err(parley_sql_core::Error::SqlHelper("cancelled".to_owned())),
        });
        assert!(w.transfer.running.is_none());
        assert!(!path.exists(), "nothing of it left");
        assert!(
            w.messages
                .iter()
                .any(|m| m.text().starts_with("Download stopped"))
        );
        assert!(w.error_line.is_none(), "stopped, not failed");
    }

    #[test]
    fn a_table_download_takes_its_columns_from_the_first_rows_with_headings_r_sql_71() {
        let scratch = Scratch::new("table");
        let path = scratch.0.join("kunder.csv");
        let mut w = window();
        let table = Source::Table {
            library: "LIB".to_owned(),
            name: "KUNDER".to_owned(),
        };
        w.start_download(choice(table, true), Some(&PicksFile(path.clone())));
        assert_eq!(w.transfer.progress(), Some(0), "waiting for the first rows");
        w.on_download_run(answer(serde_json::json!({
            "kind": "rows", "cursor": 3, "more": false,
            "columns": [column_json("NAVN")], "rows": [["Søren"]],
        })));
        let text = std::fs::read_to_string(&path).expect("the file");
        assert_eq!(text, "\u{FEFF}NAVN heading\r\nSøren\r\n");
    }

    #[test]
    fn an_error_from_the_host_removes_the_file_and_says_why_r_sql_71() {
        let scratch = Scratch::new("error");
        let path = scratch.0.join("t.csv");
        let mut w = window();
        w.results.add(
            "SELECT NAVN FROM T",
            ResultView::new(vec![column("NAVN")], rows(&["a"]), true, 7),
        );
        w.start_download(choice(Source::Tab, false), Some(&PicksFile(path.clone())));
        w.on_download_fetch(Event {
            tag: 0,
            result: Err(parley_sql_core::Error::SqlHelper(
                "the host went away".to_owned(),
            )),
        });
        assert!(!path.exists());
        assert!(
            w.error_line
                .as_deref()
                .is_some_and(|e| e.contains("the host went away"))
        );
    }

    #[test]
    fn a_table_download_waits_while_a_statement_runs_r_sql_71() {
        let scratch = Scratch::new("busy");
        let path = scratch.0.join("kunder.csv");
        let mut w = window();
        crate::tests::running(&mut w, "SELECT * FROM ÆØÅ");
        let table = Source::Table {
            library: "LIB".to_owned(),
            name: "KUNDER".to_owned(),
        };
        w.start_download(choice(table, false), Some(&PicksFile(path.clone())));
        assert!(w.transfer.running.is_none(), "not started");
        assert_eq!(
            w.waiting,
            Some((1, Waiting::Run("SELECT * FROM ÆØÅ".to_owned()))),
            "the statement's answer is still waited for"
        );
        assert_eq!(w.error_line.as_deref(), Some(crate::WAIT));
        assert!(!path.exists());
    }

    #[test]
    fn the_download_dialog_says_to_wait_while_a_statement_runs_r_sql_71() {
        use egui_kittest::kittest::{NodeT, Queryable};
        let mut w = window().signed_on_for_tests();
        crate::tests::running(&mut w, "VALUES 1");
        w.transfer.ask(Source::Table {
            library: "LIB".to_owned(),
            name: "KUNDER".to_owned(),
        });
        let mut harness = drawn(w, Vec::new());
        harness.run_steps(2);
        harness.get_by_label(crate::WAIT);
        assert!(
            harness
                .get_by_label("Download…")
                .accesskit_node()
                .is_disabled()
        );
    }

    #[test]
    fn the_download_dialog_keeps_its_buttons_in_place_r_sql_71() {
        use egui_kittest::kittest::Queryable;
        let mut w = window().signed_on_for_tests();
        w.transfer.ask(Source::Tab);
        let mut harness = drawn(w, Vec::new());
        harness.run_steps(2);
        harness.get_by_label("Excel").click();
        harness.run_steps(2);
        let button = harness.get_by_label("Download…").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Download…").rect(), button, "D12");
        assert_eq!(
            harness.state().transfer.dialog.as_ref().map(|c| c.format),
            Some(Format::Excel)
        );
        harness
            .get_all_by_label("Cancel")
            .last()
            .expect("the dialog's Cancel")
            .click();
        harness.run_steps(2);
        assert!(harness.state().transfer.dialog.is_none());
    }

    fn backup(path: PathBuf, whole: bool) -> browser::Backup {
        browser::Backup {
            path,
            library: "LIB".to_owned(),
            before: "-- head\n".to_owned(),
            after: "\n-- Indexes\n".to_owned(),
            tables: vec![
                DataTable {
                    name: "A".to_owned(),
                    overriding: false,
                },
                DataTable {
                    name: "Ø".to_owned(),
                    overriding: true,
                },
            ],
            whole,
        }
    }

    fn rows_answer(name: &str, cursor: u64, more: bool, rows: &[&str]) -> Event {
        answer(serde_json::json!({
            "kind": "rows", "cursor": cursor, "more": more,
            "columns": [column_json(name)], "rows": rows.iter().map(|r| [r]).collect::<Vec<_>>(),
        }))
    }

    #[test]
    fn a_backup_writes_its_sql_then_each_tables_rows_then_the_rest_r_sql_96() {
        let scratch = Scratch::new("backup");
        let path = scratch.0.join("lib.sql");
        let mut w = window();
        w.start_backup(backup(path.clone(), true));
        assert!(w.busy(), "the backup holds the connection");
        w.on_download_run(rows_answer("N", 1, true, &["1"]));
        w.on_download_fetch(answer(serde_json::json!({"rows": [["2"]], "more": false})));
        assert!(
            w.transfer.running.is_some(),
            "the second table is read next"
        );
        w.on_download_run(rows_answer("ID", 2, false, &["æøå"]));
        assert!(w.transfer.running.is_none(), "done");
        let text = std::fs::read_to_string(&path).expect("the file");
        assert_eq!(
            text,
            "-- head\n\
             \n-- The rows of LIB.A\n\
             INSERT INTO LIB.A (N) VALUES ('1');\n\
             INSERT INTO LIB.A (N) VALUES ('2');\n\
             \n-- The rows of LIB.\"Ø\"\n\
             INSERT INTO LIB.\"Ø\" (ID) OVERRIDING SYSTEM VALUE VALUES ('æøå');\n\
             \n-- Indexes\n"
        );
        assert!(
            w.messages
                .iter()
                .any(|m| m.text().starts_with("Backed up 2 tables, 3 rows")),
            "{:?}",
            w.messages
        );
    }

    #[test]
    fn a_stopped_backup_reads_no_more_tables_and_leaves_no_file_r_sql_96() {
        let scratch = Scratch::new("backup-cancel");
        let path = scratch.0.join("lib.sql");
        let mut w = window();
        w.start_backup(backup(path.clone(), true));
        w.cancel();
        // The first table's rows came before the cancel reached the host.
        w.on_download_run(rows_answer("N", 1, false, &["1"]));
        assert!(w.transfer.running.is_none(), "the second table is not read");
        assert!(!path.exists(), "nothing of it left");
        assert!(w.error_line.is_none(), "stopped, not failed");
    }

    /// The owner, 2026-11-15: after an SQL download failed, a CSV one that worked still showed
    /// the old error.
    #[test]
    fn a_download_that_starts_clears_an_earlier_failure_r_sql_71() {
        let scratch = Scratch::new("clears");
        let path = scratch.0.join("kunder.csv");
        let mut w = window();
        w.fail("Not downloaded: SQL0104".to_owned());
        let table = Source::Table {
            library: "LIB".to_owned(),
            name: "KUNDER".to_owned(),
        };
        w.start_download(choice(table, false), Some(&PicksFile(path.clone())));
        assert_eq!(w.error_line, None);
        let mut w = window();
        w.fail("Not backed up: SQL0104".to_owned());
        w.start_backup(backup(scratch.0.join("lib.sql"), true));
        assert_eq!(w.error_line, None);
    }

    #[test]
    fn a_backup_waits_while_a_statement_runs_r_sql_96() {
        let scratch = Scratch::new("backup-busy");
        let path = scratch.0.join("lib.sql");
        let mut w = window();
        crate::tests::running(&mut w, "SELECT * FROM ÆØÅ");
        w.start_backup(backup(path.clone(), true));
        assert!(w.transfer.running.is_none(), "not started");
        assert!(!path.exists());
        assert!(
            w.error_line
                .as_deref()
                .is_some_and(|e| e.starts_with("Not backed up: Wait"))
        );
    }

    #[test]
    fn one_table_as_sql_asks_the_catalog_then_says_downloaded_r_sql_95() {
        let scratch = Scratch::new("table-sql");
        let path = scratch.0.join("kunder.sql");
        let mut w = window();
        let table = Source::Table {
            library: "LIB".to_owned(),
            name: "Ø".to_owned(),
        };
        let choice = Choice {
            source: table,
            format: Format::Sql,
            headings: false,
        };
        w.start_download(choice, Some(&PicksFile(path.clone())));
        assert!(
            w.transfer.running.is_none(),
            "the catalog is asked first whether its rows override"
        );
        let mut one = backup(path.clone(), false);
        one.tables.remove(0);
        one.after = String::new();
        w.use_pick(browser::Pick::Backup(one));
        w.on_download_run(rows_answer("ID", 2, false, &["7"]));
        let text = std::fs::read_to_string(&path).expect("the file");
        assert_eq!(
            text,
            "-- head\nINSERT INTO LIB.\"Ø\" (ID) OVERRIDING SYSTEM VALUE VALUES ('7');\n"
        );
        assert!(
            w.messages
                .iter()
                .any(|m| m.text().starts_with("Downloaded 1 rows"))
        );
    }

    #[test]
    fn the_download_dialog_offers_sql_insert_without_headings_r_sql_95() {
        use egui_kittest::kittest::Queryable;
        let mut w = window().signed_on_for_tests();
        w.transfer.ask(Source::Tab);
        let mut harness = drawn(w, Vec::new());
        harness.run_steps(2);
        harness.get_by_label("SQL INSERT").click();
        harness.run_steps(2);
        let button = harness.get_by_label("Download…").rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Download…").rect(), button, "D12");
        assert!(
            harness
                .query_by_label("IBM i column headings as the first row")
                .is_none()
        );
        assert_eq!(
            harness.state().transfer.dialog.as_ref().map(|c| c.format),
            Some(Format::Sql)
        );
    }
}
