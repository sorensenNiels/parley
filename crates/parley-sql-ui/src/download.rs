//! Download every row (R-SQL-71, R-SQL-72, D31): the dialog, then the rows written to the file
//! as they come, from a result tab's open cursor or from a table read whole.

use egui::{Context, Ui};
use parley_sql_core::catalog;
use parley_sql_core::protocol::{Column, MoreRows, Outcome, Request, Row};
use parley_sql_core::runner::Event;
use parley_sql_core::transfer::{self, Format};

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
}

/// The download dialog and the download under way.
#[derive(Debug, Default)]
pub struct Transfer {
    dialog: Option<Choice>,
    pub running: Option<Downloading>,
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
        match choice.source {
            Source::Tab => {
                let Some((columns, rows, cursor)) = self.results.take_for_download() else {
                    return;
                };
                let sheet = transfer::suggested_name(&columns);
                let names = names(&columns, choice.headings);
                match transfer::Download::create(&path, choice.format, &columns, &names, &sheet) {
                    Ok(writer) => {
                        self.transfer.running = Some(Downloading {
                            writer: Some(writer),
                            pending: None,
                            cursor,
                            cancelled: false,
                        });
                        self.download_rows(&rows, cursor.is_some());
                    }
                    Err(e) => self.fail(e.to_string()),
                }
            }
            Source::Table { library, name } => {
                self.transfer.running = Some(Downloading {
                    writer: None,
                    pending: Some((path, choice.format, choice.headings, name.clone())),
                    cursor: None,
                    cancelled: false,
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
            Some(_) | None => self.end_download(None),
        }
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
                    return;
                };
                running.cursor = more.then_some(cursor);
                let names = names(&columns, headings);
                match transfer::Download::create(&path, format, &columns, &names, &sheet) {
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
        if let Some(e) = error {
            writer.abandon();
            return self.fail(format!("Not downloaded: {e}"));
        }
        match writer.finish() {
            Ok(rows) => self
                .messages
                .push(Note::Info(format!("Downloaded {rows} rows to {path}."))),
            Err(e) => self.fail(format!("Not downloaded: {e}")),
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
        &[(Format::Csv, "CSV"), (Format::Excel, "Excel")],
    );
    ui.add_space(8.0);
    theme::switch_row(
        ui,
        "IBM i column headings as the first row",
        &mut choice.headings,
    );
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
}
