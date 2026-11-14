//! IBM i tools in the SQL window: Messages with an error's help (its second-level text) on
//! asking, the SQL job's job log as a result (R-SQL-31), and its current schema and library
//! list in the top bar (R-SQL-32).

use std::collections::BTreeMap;

use egui::{RichText, ScrollArea, Ui};
use parley_core::sql::protocol::{Answer, Outcome, Request};
use parley_core::sql::runner::Event;
use parley_core::sql::{catalog, message, text};

use super::grid::ResultView;
use super::{SqlWindow, Stage, Tab, Waiting};
use crate::{icons, theme};

/// Seconds the window's own queries may take.
const TIMEOUT: u32 = 30;

/// The SQL job's current schema and library list (R-SQL-32).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobContext {
    pub schema: String,
    /// (library, kind), in order.
    pub libraries: Vec<(String, String)>,
}

/// An error's help, by its place in Messages.
pub type Helps = BTreeMap<usize, String>;

impl SqlWindow {
    /// Row counts, warnings and errors; Explain on an error with a message ID, and Job log.
    pub(super) fn messages(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none() && self.queue.is_empty();
        let mut explain = None;
        let mut job_log = false;
        ui.horizontal(|ui| {
            theme::button_row(ui, |ui| {
                let button = icons::button(icons::LIST, "Job log");
                job_log = icons::add_named(ui, idle, button, "Job log")
                    .on_hover_text("The SQL job's newest messages, as a result")
                    .clicked();
            });
        });
        if self.messages.is_empty() {
            theme::hint(ui, "Row counts, warnings and errors appear here.");
        }
        ScrollArea::vertical()
            .id_salt("sql-messages")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for (i, (error, text)) in self.messages.iter().enumerate() {
                    if !*error {
                        ui.label(RichText::new(text).monospace().small());
                        continue;
                    }
                    // "statement: [SQL0204] …": the ID after the statement.
                    let id = text
                        .rfind(": [")
                        .and_then(|at| text.get(at + 2..))
                        .and_then(message::message_id);
                    ui.horizontal_wrapped(|ui| {
                        theme::error(ui, text);
                        if let Some(id) = id
                            && !self.helps.contains_key(&i)
                        {
                            let name = format!("Explain {id}");
                            let button = egui::Button::new("Explain").small();
                            if icons::add_named(ui, idle, button, &name)
                                .on_hover_text("Its cause and recovery, from the job log")
                                .clicked()
                            {
                                explain = Some((i, id.to_owned()));
                            }
                        }
                    });
                    if let Some(help) = self.helps.get(&i) {
                        theme::card(ui).show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(help).monospace().small()).wrap(),
                            );
                        });
                    }
                }
            });
        if let Some((index, id)) = explain {
            let sql = message::from_job_log(&id);
            self.ask(
                sql,
                2,
                Waiting::Explain {
                    index,
                    id,
                    file: false,
                },
            );
        } else if job_log {
            self.ask(message::JOB_LOG.to_owned(), 101, Waiting::JobLog);
        }
    }

    /// Runs one of the window's own queries; it is not in the history.
    fn ask(&mut self, sql: String, max_rows: u32, waiting: Waiting) {
        let run = Request::Run {
            sql,
            max_rows,
            timeout: TIMEOUT,
            params: Vec::new(),
        };
        self.send(run, waiting);
    }

    /// The help for message `id` at `index`: from the job log, else from its message file.
    pub(super) fn on_explain(&mut self, index: usize, id: &str, file: bool, event: Event) {
        let first = match event.result.and_then(Answer::into_result::<Outcome>) {
            Ok(Outcome::Rows { rows, .. }) => rows.into_iter().next(),
            Ok(Outcome::Count { .. }) => None,
            Err(e) => {
                self.helps.insert(index, format!("No help: {e}"));
                return;
            }
        };
        match first {
            Some(row) => {
                let text = |n: usize| row.get(n).cloned().flatten().unwrap_or_default();
                let help = format!("{id}: {}\n\n{}", text(0), message::second_level(&text(1)));
                self.helps.insert(index, help.trim().to_owned());
            }
            None if !file => {
                let sql = message::from_message_file(id);
                self.ask(
                    sql,
                    2,
                    Waiting::Explain {
                        index,
                        id: id.to_owned(),
                        file: true,
                    },
                );
            }
            None => {
                self.helps.insert(
                    index,
                    format!("{id} is not in the job log or its message file."),
                );
            }
        }
    }

    /// The job log as a result tab.
    pub(super) fn on_job_log(&mut self, event: Event) {
        match event.result.and_then(Answer::into_result::<Outcome>) {
            Ok(Outcome::Rows {
                cursor,
                columns,
                rows,
                more,
            }) => {
                self.results
                    .add("Job log", ResultView::new(columns, rows, more, cursor));
                self.tab = Tab::Results;
            }
            Ok(Outcome::Count { .. }) => {}
            Err(e) => self.fail(format!("Job log: {e}")),
        }
    }

    /// Reads the current schema and library list when they may have changed and nothing runs.
    pub(super) fn read_job_when_idle(&mut self) {
        let idle = self.waiting.is_none() && self.queue.is_empty() && self.confirm.is_none();
        if self.job_stale && idle && self.stage == Stage::Ready {
            self.job_stale = false;
            self.ask(catalog::JOB.to_owned(), 1000, Waiting::Job);
        }
    }

    pub(super) fn on_job(&mut self, event: Event) {
        // Only for the top bar: when it cannot be read, the menu says so.
        let answer = event.result.and_then(Answer::into_result::<Outcome>);
        if let Ok(Outcome::Rows { rows, .. }) = answer {
            let text = |row: &[Option<String>], n: usize| {
                row.get(n)
                    .cloned()
                    .flatten()
                    .unwrap_or_default()
                    .trim()
                    .to_owned()
            };
            self.job = Some(JobContext {
                schema: rows.first().map(|r| text(r, 0)).unwrap_or_default(),
                libraries: rows.iter().map(|r| (text(r, 1), text(r, 2))).collect(),
            });
        }
    }

    /// The current schema, and a menu with the library list: a click makes a library the
    /// current schema, and a library can be added (R-SQL-32). Both run as statements, so they
    /// are in the history.
    pub(super) fn schema_menu(&mut self, ui: &mut Ui) {
        let idle = self.waiting.is_none() && self.queue.is_empty();
        let schema = self.job.as_ref().map_or("…", |j| j.schema.as_str());
        let label = format!("Schema {schema}");
        let mut run = None;
        ui.add_enabled_ui(idle, |ui| {
            ui.menu_button(label, |ui| run = self.library_list(ui))
                .response
                .on_hover_text(
                    "The SQL job's current schema: where unqualified tables are looked for",
                );
        });
        if let Some(sql) = run {
            self.run_now(&sql);
        }
    }

    /// The schema menu's content; the statement to run, if one was chosen.
    fn library_list(&mut self, ui: &mut Ui) -> Option<String> {
        let mut run = None;
        ui.set_min_width(260.0);
        theme::section_label(ui, "Library list");
        match &self.job {
            Some(job) => {
                for (library, kind) in &job.libraries {
                    let current = *library == job.schema;
                    let item = egui::Button::new(theme::semibold(library))
                        .right_text(RichText::new(kind).small())
                        .frame_when_inactive(false)
                        .selected(current);
                    let response = ui
                        .add_sized([ui.available_width(), 22.0], item)
                        .on_hover_text("Make it the current schema");
                    // A name from the host goes into SQL only when it is a system name.
                    if response.clicked()
                        && !current
                        && let Some(name) = text::system_name(library)
                    {
                        run = Some(format!("SET SCHEMA {name}"));
                        ui.close();
                    }
                }
            }
            None => theme::hint(ui, "Not read yet."),
        }
        ui.separator();
        ui.horizontal(|ui| {
            let label = ui.label("Add");
            let field = egui::TextEdit::singleline(&mut self.add_library).hint_text("Library");
            theme::sized_edit(ui, 120.0, field).labelled_by(label.id);
            let name = text::system_name(&self.add_library);
            let add = ui.add_enabled(name.is_some(), egui::Button::new("Add to library list"));
            if add.clicked()
                && let Some(name) = name
            {
                run = Some(format!("CL: ADDLIBLE {name}"));
                self.add_library.clear();
                ui.close();
            }
        });
        run
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{answer, drawn, window};
    use super::*;

    fn failed(w: &mut SqlWindow) {
        w.waiting = Some((1, Waiting::Run("SELECT * FROM T".to_owned())));
        answer(
            w,
            r#"{"id":1,"error":{"message":"[SQL0204] T in QGPL type *FILE not found.","sqlstate":"42704","sqlcode":-204}}"#,
        );
    }

    #[test]
    fn explain_reads_the_job_log_and_falls_back_to_the_message_file_r_sql_31() {
        let mut w = window();
        failed(&mut w);
        w.waiting = Some((
            1,
            Waiting::Explain {
                index: 0,
                id: "SQL0204".to_owned(),
                file: false,
            },
        ));
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":1,"columns":[],"rows":[["T in QGPL type *FILE not found.","Cause . . :   T æøå. &N Recovery  . . :   Change it."]],"more":false}}"#,
        );
        assert_eq!(
            w.helps.get(&0).map(String::as_str),
            Some(
                "SQL0204: T in QGPL type *FILE not found.\n\nCause . . :   T æøå.\nRecovery  . . :   Change it."
            )
        );
        w.helps.clear();
        w.waiting = Some((
            1,
            Waiting::Explain {
                index: 0,
                id: "SQL0204".to_owned(),
                file: false,
            },
        ));
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":1,"columns":[],"rows":[],"more":false}}"#,
        );
        assert!(w.helps.is_empty(), "asks the message file next");
        w.waiting = Some((
            1,
            Waiting::Explain {
                index: 0,
                id: "SQL0204".to_owned(),
                file: true,
            },
        ));
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":1,"columns":[],"rows":[],"more":false}}"#,
        );
        assert_eq!(
            w.helps.get(&0).map(String::as_str),
            Some("SQL0204 is not in the job log or its message file.")
        );
        assert_eq!(
            w.take_ran(),
            ["SELECT * FROM T"],
            "the window's own queries are not in the history"
        );
    }

    #[test]
    fn the_job_log_is_a_result_tab_r_sql_31() {
        let mut w = window();
        w.waiting = Some((1, Waiting::JobLog));
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":1,"columns":[{"name":"MESSAGE_ID","type":"VARCHAR","length":7,"scale":0,"nullable":true,"numeric":false}],"rows":[["SQL0204"]],"more":false}}"#,
        );
        assert_eq!(w.tab, Tab::Results);
        assert_eq!(w.results.shown_view().map(ResultView::counts), Some((1, 1)));
    }

    #[test]
    fn an_error_with_a_message_id_offers_explain_and_job_log_is_there_r_sql_31() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        failed(&mut w);
        w.tab = Tab::Messages;
        w.messages.push((true, "VALUES 1: no ID here".to_owned()));
        let mut harness = drawn(w, Vec::new());
        harness.run();
        harness.get_by_label("Job log");
        harness.get_by_label("Explain SQL0204");
        assert_eq!(harness.query_all_by_label_contains("Explain").count(), 1);
    }

    #[test]
    fn the_job_is_read_after_connect_and_after_set_or_cl_when_idle_r_sql_32() {
        let mut w = window();
        w.job_stale = true;
        w.read_job_when_idle();
        assert!(!w.job_stale, "asked (without a helper nothing is sent)");
        w.waiting = Some((1, Waiting::Run("SET SCHEMA MSGSUDTA".to_owned())));
        answer(&mut w, r#"{"id":1,"ok":{"kind":"count","count":0}}"#);
        assert!(!w.job_stale, "read again at once, the window being idle");
        w.waiting = Some((1, Waiting::Job));
        answer(
            &mut w,
            r#"{"id":1,"ok":{"kind":"rows","cursor":1,"columns":[],"rows":[["MSGSUDTA","QSYS","SYSTEM"],["MSGSUDTA","MSGSUDTA","USER"],["MSGSUDTA","ÆØÅ ",null]],"more":false}}"#,
        );
        let job = w.job.clone().expect("read");
        assert_eq!(job.schema, "MSGSUDTA");
        assert_eq!(job.libraries[1], ("MSGSUDTA".to_owned(), "USER".to_owned()));
        assert_eq!(job.libraries[2], ("ÆØÅ".to_owned(), String::new()));
    }

    #[test]
    fn the_schema_menu_lists_the_library_list_r_sql_32() {
        use egui_kittest::kittest::Queryable;
        let mut w = window();
        w.job = Some(JobContext {
            schema: "MSGSUDTA".to_owned(),
            libraries: vec![
                ("QSYS".to_owned(), "SYSTEM".to_owned()),
                ("MSGSUDTA".to_owned(), "USER".to_owned()),
            ],
        });
        w.connected = Some(
            serde_json::from_str(r#"{"user":"USR","release":"V7R3M0","job":"1/QUSER/QZDASOINIT"}"#)
                .expect("connected"),
        );
        let mut harness = drawn(w, Vec::new());
        harness.run();
        harness.get_by_label("Schema MSGSUDTA").click();
        harness.run();
        harness.get_by_label_contains("QSYS");
        harness.get_by_label("Add to library list");
    }
}
