//! Upload a CSV file into a table, all or nothing (R-SQL-73, D31): the dialog with its preview,
//! then the rows sent in batches inside one transaction, committed only when all went in.

use egui::{Context, Grid, RichText, ScrollArea, TextEdit, Ui};
use parley_core::sql::catalog::ColumnInfo;
use parley_core::sql::protocol::{Done, Inserted, Request};
use parley_core::sql::runner::Event;
use parley_core::sql::upload::{self, Plan, Separator};

use super::{SqlFiles, SqlWindow, Waiting};
use crate::{dialog, theme};

/// Rows the preview shows.
const PREVIEW: usize = 10;

/// The upload dialog's state.
#[derive(Debug, Default)]
pub struct Dialog {
    library: String,
    table: String,
    /// The file's path and text.
    file: Option<(String, String)>,
    names_first: bool,
    separator: Separator,
    /// The file read with `separator`; recomputed when either changes.
    records: Option<Result<Vec<Vec<String>>, String>>,
    read_with: Option<Separator>,
    /// Why the file could not be opened.
    file_error: Option<String>,
}

/// An upload under way.
#[derive(Debug)]
pub struct Uploading {
    table: String,
    plan: Plan,
    columns: Vec<ColumnInfo>,
    /// The rows to insert, the names row left out.
    records: Vec<Vec<String>>,
    /// The file's line number (from 1) of the first of `records`, for messages.
    first_line: usize,
    /// Rows sent and inserted so far.
    sent: usize,
    cancelled: bool,
}

/// The upload dialog and the upload under way.
#[derive(Debug, Default)]
pub struct Upload {
    dialog: Option<Dialog>,
    pub running: Option<Uploading>,
}

impl Upload {
    /// Opens the dialog, for `library.table` when given.
    pub fn ask(&mut self, library: &str, table: &str) {
        self.dialog = Some(Dialog {
            library: library.to_owned(),
            table: table.to_owned(),
            names_first: true,
            ..Dialog::default()
        });
    }

    /// (rows inserted, of how many), while an upload runs.
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.running.as_ref().map(|u| (u.sent, u.records.len()))
    }
}

impl Dialog {
    /// The file read with the separator chosen, read again when it changed.
    fn records(&mut self) -> Option<&Result<Vec<Vec<String>>, String>> {
        let (_, text) = self.file.as_ref()?;
        if self.read_with != Some(self.separator) {
            self.records = Some(upload::parse(text, self.separator).map_err(|e| e.to_string()));
            self.read_with = Some(self.separator);
        }
        self.records.as_ref()
    }

    /// The file chosen: its separator is found from its first line.
    fn take_file(&mut self, path: String, text: String) {
        self.separator = Separator::found_in(&text);
        self.file = Some((path, text));
        self.read_with = None;
        self.file_error = None;
    }
}

/// What the dialog can build an upload from: the plan, or why not.
fn plan(
    records: &[Vec<String>],
    names_first: bool,
    columns: &[ColumnInfo],
) -> Result<Plan, String> {
    let width = records.iter().map(Vec::len).max().unwrap_or(0);
    let names = names_first.then(|| records.first().cloned().unwrap_or_default());
    let data = records.len() - usize::from(names_first && !records.is_empty());
    if data == 0 {
        return Err("The file has no rows to upload.".to_owned());
    }
    let plan = Plan::new(columns, names.as_deref(), width);
    match plan.problem() {
        Some(problem) => Err(problem),
        None => Ok(plan),
    }
}

/// The host's message for a failed upload, with what SQL7008 means here.
fn explain(error: &parley_core::sql::protocol::SqlError) -> String {
    if error.sqlcode == -7008 {
        format!(
            "{error}. The table is not journaled: an all-or-nothing upload needs commitment control, which needs a journal."
        )
    } else {
        error.to_string()
    }
}

impl SqlWindow {
    /// The upload dialog while open (R-SQL-73).
    pub(super) fn upload_dialog(&mut self, ctx: &Context, files: Option<&dyn SqlFiles>) {
        let Some(dialog) = self.upload.dialog.as_mut() else {
            return;
        };
        let columns =
            self.browser
                .table_columns(&dialog.library, &dialog.table, &mut self.database);
        let busy = self.waiting.is_some() || !self.queue.is_empty();
        let blocked = if self.uncommitted {
            Some("Commit or roll back the changes first: the upload commits when it ends.")
        } else if busy {
            Some("Wait for the statement running to end.")
        } else {
            None
        };
        let (close, (cancel, start, choose)) =
            dialog::dialog(ctx, "sql-upload", "Upload CSV", 640.0, |ui| {
                let columns = columns
                    .as_ref()
                    .map(|r| r.as_ref().map(|(_, c)| c.as_slice()));
                upload_content(ui, dialog, columns, blocked)
            });
        if choose && let Some(files) = files {
            match files.open_csv() {
                Ok(Some((path, text))) => dialog.take_file(path, text),
                Ok(None) => {}
                Err(e) => dialog.file_error = Some(e),
            }
        }
        if close || cancel {
            self.upload.dialog = None;
        } else if start && let Some(Ok((table, columns))) = columns {
            self.start_upload(&table, columns);
        }
    }

    /// Starts uploading into `table` (its SQL name) of the dialog's library.
    fn start_upload(&mut self, table: &str, columns: Vec<ColumnInfo>) {
        let Some(mut dialog) = self.upload.dialog.take() else {
            return;
        };
        let names_first = dialog.names_first;
        let Some(Ok(records)) = dialog.records().cloned() else {
            return;
        };
        let plan = match plan(&records, names_first, &columns) {
            Ok(plan) => plan,
            Err(e) => return self.fail(e),
        };
        let library = parley_core::sql::text::system_name(&dialog.library).unwrap_or_default();
        let sql = plan.insert(&library, table, &columns);
        let skip = usize::from(names_first);
        self.upload.running = Some(Uploading {
            table: parley_core::sql::catalog::qualified(&library, table),
            plan,
            columns,
            records: records.into_iter().skip(skip).collect(),
            first_line: 1 + skip,
            sent: 0,
            cancelled: false,
        });
        self.send(Request::InsertBegin { sql }, Waiting::UploadBegin);
    }

    /// The next batch of rows, or the end: commit when all went in, roll back when cancelled.
    fn upload_next(&mut self) {
        let Some(running) = self.upload.running.as_ref() else {
            return;
        };
        if running.cancelled {
            return self.send(
                Request::InsertEnd { commit: false },
                Waiting::UploadEnd(false),
            );
        }
        if running.sent >= running.records.len() {
            return self.send(
                Request::InsertEnd { commit: true },
                Waiting::UploadEnd(true),
            );
        }
        let end = (running.sent + upload::BATCH).min(running.records.len());
        let rows = running
            .records
            .get(running.sent..end)
            .unwrap_or_default()
            .iter()
            .map(|r| running.plan.values(r, &running.columns))
            .collect();
        self.send(
            Request::InsertRows { rows },
            Waiting::UploadRows(end - running.sent),
        );
    }

    /// The answer to the upload's start.
    pub(super) fn on_upload_begin(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Done>)
        {
            Ok(_) => self.upload_next(),
            Err(e) => {
                self.upload.running = None;
                self.fail(format!("Not uploaded: {e}"));
            }
        }
    }

    /// The answer to a batch of `count` rows. A failure was rolled back by the helper.
    pub(super) fn on_upload_rows(&mut self, count: usize, event: Event) {
        let answer = event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Inserted>);
        let Some(running) = self.upload.running.as_mut() else {
            return;
        };
        match answer {
            Ok(Inserted { failure: None, .. }) => {
                running.sent += count;
                self.upload_next();
            }
            Ok(Inserted {
                failure: Some(failure),
                ..
            }) => {
                let running = self.upload.running.take().expect("checked above");
                if running.cancelled {
                    return self.upload_stopped();
                }
                let line = failure
                    .index
                    .and_then(|i| usize::try_from(i).ok())
                    .map(|i| format!("line {} of the file", running.first_line + running.sent + i));
                let at = line.map_or_else(String::new, |l| format!(" at {l}"));
                self.fail(format!(
                    "Not uploaded: nothing went into {}. It failed{at}: {}",
                    running.table,
                    explain(&failure.error)
                ));
            }
            Err(e) => {
                // The helper could not be asked or answer: end the upload, rolled back.
                let cancelled = running.cancelled;
                self.upload.running = None;
                self.send(
                    Request::InsertEnd { commit: false },
                    Waiting::UploadEnd(false),
                );
                if cancelled {
                    self.upload_stopped();
                } else {
                    self.fail(format!("Not uploaded: {e}"));
                }
            }
        }
    }

    /// The answer to the upload's end: committed, or rolled back.
    pub(super) fn on_upload_end(&mut self, commit: bool, event: Event) {
        let result = event
            .result
            .and_then(parley_core::sql::protocol::Answer::into_result::<Done>);
        let Some(running) = self.upload.running.take() else {
            return;
        };
        match (result, commit) {
            (Ok(_), true) => {
                self.messages.push((
                    false,
                    format!("Uploaded {} rows into {}.", running.sent, running.table),
                ));
                self.tab = super::Tab::Messages;
            }
            (Ok(_), false) => self.upload_stopped(),
            (Err(e), _) => self.fail(format!(
                "Not uploaded: nothing went into {}: {}",
                running.table,
                match e {
                    parley_core::Error::Sql(sql) => explain(&sql),
                    other => other.to_string(),
                }
            )),
        }
    }

    fn upload_stopped(&mut self) {
        self.messages.push((
            false,
            "Upload stopped: rolled back, nothing went in.".to_owned(),
        ));
    }

    /// Cancel: no more batches; the transaction is rolled back.
    pub(super) fn cancel_upload(&mut self) {
        if let Some(running) = self.upload.running.as_mut() {
            running.cancelled = true;
        }
    }
}

/// The dialog's content; (Cancel, Upload, Choose file).
fn upload_content(
    ui: &mut Ui,
    dialog: &mut Dialog,
    columns: Option<Result<&[ColumnInfo], &String>>,
    blocked: Option<&str>,
) -> (bool, bool, bool) {
    let mut choose = false;
    Grid::new("sql-upload-fields")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Table");
            ui.horizontal(|ui| {
                theme::sized_edit(
                    ui,
                    140.0,
                    TextEdit::singleline(&mut dialog.library).hint_text("Library"),
                );
                ui.label(".");
                theme::sized_edit(
                    ui,
                    220.0,
                    TextEdit::singleline(&mut dialog.table).hint_text("Table"),
                );
            });
            ui.end_row();
            ui.label("File");
            ui.horizontal(|ui| {
                choose = ui.button("Choose…").clicked();
                match &dialog.file {
                    Some((path, _)) => ui.label(RichText::new(path).monospace()),
                    None => ui.label(RichText::new("No file chosen").weak()),
                };
            });
            ui.end_row();
        });
    if let Some(e) = &dialog.file_error {
        theme::error(ui, e);
    }
    ui.add_space(6.0);
    theme::switch_row(ui, "First row is column names", &mut dialog.names_first);
    ui.horizontal(|ui| {
        ui.label("Separator");
        let options: Vec<(Separator, &str)> =
            Separator::ALL.iter().map(|s| (*s, s.label())).collect();
        theme::segmented(ui, &mut dialog.separator, &options);
    });
    ui.add_space(8.0);
    let names_first = dialog.names_first;
    let ready = preview_and_plan(ui, dialog.records(), names_first, columns);
    theme::hint(
        ui,
        "Every row goes in one transaction: if one fails, nothing is kept. The table must be journaled.",
    );
    if let Some(why) = blocked {
        theme::warning(ui, why);
    }
    let (cancel, start) = dialog::actions(
        ui,
        theme::primary_button(ui, "Upload"),
        ready && blocked.is_none(),
    );
    (cancel, start, choose)
}

/// The preview of the first rows under the table's columns, and what the plan says; true when
/// the upload can start.
fn preview_and_plan(
    ui: &mut Ui,
    records: Option<&Result<Vec<Vec<String>>, String>>,
    names_first: bool,
    columns: Option<Result<&[ColumnInfo], &String>>,
) -> bool {
    let records = match records {
        None => {
            preview_frame(ui, |ui| theme::hint(ui, "Choose a CSV file (UTF-8)."));
            return false;
        }
        Some(Err(e)) => {
            preview_frame(ui, |ui| theme::error(ui, e));
            return false;
        }
        Some(Ok(records)) => records,
    };
    let columns = match columns {
        None => {
            preview_frame(ui, |ui| theme::hint(ui, "Reading the table's columns…"));
            return false;
        }
        Some(Err(e)) => {
            preview_frame(ui, |ui| theme::error(ui, e));
            return false;
        }
        Some(Ok(columns)) => columns,
    };
    let plan = plan(records, names_first, columns);
    let shown = match &plan {
        Ok(plan) => plan.clone(),
        Err(_) => Plan::new(
            columns,
            names_first
                .then(|| records.first().cloned().unwrap_or_default())
                .as_deref(),
            records.iter().map(Vec::len).max().unwrap_or(0),
        ),
    };
    let data: Vec<&Vec<String>> = records
        .iter()
        .skip(usize::from(names_first))
        .take(PREVIEW)
        .collect();
    preview_frame(ui, |ui| {
        Grid::new("sql-upload-preview")
            .striped(true)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                for &(_, t) in &shown.pairs {
                    if let Some(c) = columns.get(t) {
                        ui.label(theme::semibold(&c.name));
                    }
                }
                ui.end_row();
                for record in &data {
                    for &(f, _) in &shown.pairs {
                        let value = record.get(f).map_or("", String::as_str);
                        ui.label(RichText::new(value).monospace());
                    }
                    ui.end_row();
                }
            });
    });
    let rows = records.len() - usize::from(names_first && !records.is_empty());
    theme::hint(ui, format!("{rows} rows in the file."));
    if !shown.unmatched.is_empty() {
        theme::warning(
            ui,
            format!("Not in the table, left out: {}", shown.unmatched.join(", ")),
        );
    }
    if !shown.left_out.is_empty() {
        theme::hint(
            ui,
            format!("Columns the file leaves out: {}", shown.left_out.join(", ")),
        );
    }
    match plan {
        Ok(_) => true,
        Err(e) => {
            theme::error(ui, e);
            false
        }
    }
}

/// The preview's box: a fixed size, so the dialog never grows with its content (D12).
fn preview_frame(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let size = egui::vec2(600.0, 200.0);
    ui.allocate_ui(size, |ui| {
        ui.set_min_size(size);
        ScrollArea::both()
            .id_salt("sql-upload-preview")
            .max_width(size.x)
            .max_height(size.y)
            .auto_shrink([false, false])
            .show(ui, add);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sql::tests::{drawn, window};
    use parley_core::sql::protocol::Answer;

    fn column(name: &str, nullable: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_owned(),
            data_type: "VARCHAR(20)".to_owned(),
            nullable,
            has_default: false,
            heading: String::new(),
            text: String::new(),
        }
    }

    fn answer(ok: serde_json::Value) -> Event {
        Event {
            tag: 0,
            result: Ok(Answer {
                id: Some(1),
                outcome: Ok(ok),
            }),
        }
    }

    /// A window uploading `text` into LIB.KUNDER (NAVN, BYNAVN), started and begun.
    fn uploading(text: &str) -> SqlWindow {
        let mut w = window();
        w.upload.ask("lib", "kunder");
        w.upload
            .dialog
            .as_mut()
            .expect("open")
            .take_file("/tmp/k.csv".to_owned(), text.to_owned());
        w.start_upload(
            "KUNDER",
            vec![column("NAVN", false), column("BYNAVN", true)],
        );
        w.on_upload_begin(answer(serde_json::json!({})));
        w
    }

    #[test]
    fn rows_go_in_and_the_end_commits_and_says_how_many_r_sql_73() {
        let mut w = uploading("navn;bynavn\nSøren;Århus\nAnne;\n");
        assert_eq!(w.upload.progress(), Some((0, 2)), "the names row left out");
        let running = w.upload.running.as_ref().expect("running");
        assert_eq!(
            running.plan.insert("LIB", "KUNDER", &running.columns),
            "INSERT INTO LIB.KUNDER (NAVN, BYNAVN) VALUES (?, ?)"
        );
        w.on_upload_rows(2, answer(serde_json::json!({"inserted": 2})));
        assert_eq!(w.upload.progress(), Some((2, 2)));
        w.on_upload_end(true, answer(serde_json::json!({})));
        assert!(w.upload.running.is_none());
        assert!(
            w.messages
                .iter()
                .any(|(_, m)| m == "Uploaded 2 rows into LIB.KUNDER.")
        );
    }

    #[test]
    fn a_failed_row_names_its_line_and_what_sql7008_means_r_sql_73() {
        let mut w = uploading("navn,bynavn\na,b\nc,d\n");
        w.on_upload_rows(
            2,
            answer(
                serde_json::json!({"inserted": 0, "failure": {"index": 1, "error": {
                "message": "[SQL7008] KUNDER in LIB not valid for operation.",
                "sqlstate": "55019", "sqlcode": -7008}}}),
            ),
        );
        assert!(w.upload.running.is_none(), "the helper ended it");
        let error = w.error_line.clone().expect("said");
        assert!(error.contains("nothing went into LIB.KUNDER"), "{error}");
        assert!(error.contains("line 3 of the file"), "{error}");
        assert!(error.contains("not journaled"), "{error}");
    }

    #[test]
    fn cancel_rolls_back_and_says_so_r_sql_73() {
        let mut w = uploading("navn,bynavn\na,b\n");
        assert!(w.upload.running.is_some(), "started");
        w.cancel();
        // The batch running was stopped: the helper answers with a failure it rolled back.
        w.on_upload_rows(
            1,
            answer(
                serde_json::json!({"inserted": 0, "failure": {"index": null, "error": {
                "message": "cancelled", "sqlstate": null, "sqlcode": 0}}}),
            ),
        );
        assert!(w.upload.running.is_none());
        assert!(w.error_line.is_none(), "stopped, not failed");
        assert!(
            w.messages
                .iter()
                .any(|(_, m)| m.starts_with("Upload stopped"))
        );
    }

    #[test]
    fn a_file_with_only_names_or_a_needed_column_left_out_does_not_start_r_sql_73() {
        let columns = vec![column("NAVN", false), column("BY", true)];
        let records = upload::parse("navn\n", Separator::Comma).expect("parses");
        assert!(plan(&records, true, &columns).is_err_and(|e| e.contains("no rows")));
        let records = upload::parse("by\nÅrhus\n", Separator::Comma).expect("parses");
        assert!(plan(&records, true, &columns).is_err_and(|e| e.contains("NAVN")));
        let records = upload::parse("x,y\n", Separator::Comma).expect("parses");
        assert!(plan(&records, false, &columns).is_ok(), "by position");
    }

    #[test]
    fn the_upload_dialog_keeps_its_buttons_in_place_and_waits_for_a_file_r_sql_73() {
        use egui_kittest::kittest::{NodeT, Queryable};
        let mut w = window().signed_on_for_tests();
        w.upload.ask("LIB", "KUNDER");
        let mut harness = drawn(w, Vec::new());
        harness.run_steps(2);
        harness.get_by_label("Choose a CSV file (UTF-8).");
        let upload = harness.get_by_label("Upload");
        assert!(upload.accesskit_node().is_disabled(), "no file yet");
        let place = upload.rect();
        harness.run_steps(5);
        assert_eq!(harness.get_by_label("Upload").rect(), place, "D12");
        harness.state_mut().uncommitted = true;
        harness.run_steps(2);
        harness.get_by_label_contains("Commit or roll back the changes first");
        harness
            .get_all_by_label("Cancel")
            .last()
            .expect("the dialog's Cancel")
            .click();
        harness.run_steps(2);
        assert!(harness.state().upload.dialog.is_none());
    }
}
