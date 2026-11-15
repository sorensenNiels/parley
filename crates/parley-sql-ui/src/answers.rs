//! What the helper answers, by what was waited for.

use parley_sql_core::protocol::{self, Connected, Done, Hello, MoreRows, Outcome};
use parley_sql_core::runner::{Event, Runner};
use parley_sql_core::text;

use super::{Closing, Note, SqlWindow, Stage, Tab, Waiting};
use crate::editor::gutter;
use crate::grid::ResultView;

impl SqlWindow {
    /// Takes what the helper answered since the last frame.
    pub(crate) fn read_events(&mut self) {
        while let Some(event) = self.runner.as_ref().and_then(Runner::try_event) {
            self.handle(event);
        }
        self.database.read_events();
        let answers = self.database.take_answers();
        let own = [
            self.connected.as_ref().map(|c| c.job.as_str()),
            self.database.job(),
        ];
        let rest = self.system.receive(answers, own);
        for pick in self.browser.receive(rest) {
            self.use_pick(pick);
        }
    }

    pub(crate) fn handle(&mut self, event: Event) {
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
            Waiting::DownloadRun => self.on_download_run(event),
            Waiting::DownloadFetch(_) => self.on_download_fetch(event),
            Waiting::UploadBegin => self.on_upload_begin(event),
            Waiting::UploadRows(count) => self.on_upload_rows(count, event),
            Waiting::UploadEnd(commit) => self.on_upload_end(commit, event),
            Waiting::Fetch(cursor) => match event
                .result
                .and_then(parley_sql_core::protocol::Answer::into_result::<MoreRows>)
            {
                Ok(more) => self.results.append(cursor, more.rows, more.more),
                Err(e) => self.fail(e.to_string()),
            },
            Waiting::Autocommit(on) => match event
                .result
                .and_then(parley_sql_core::protocol::Answer::into_result::<Done>)
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
                    .and_then(parley_sql_core::protocol::Answer::into_result::<Done>)
                {
                    Ok(_) => {
                        self.uncommitted = false;
                        self.messages.push(Note::Info(done.to_owned()));
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

    pub(crate) fn on_hello(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_sql_core::protocol::Answer::into_result::<Hello>)
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

    pub(crate) fn on_connect(&mut self, event: Event) {
        match event
            .result
            .and_then(parley_sql_core::protocol::Answer::into_result::<Connected>)
        {
            Ok(connected) => {
                self.messages.push(Note::Info(format!(
                    "Connected as {} to {} (job {}).",
                    connected.user, self.config.host, connected.job
                )));
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

    pub(crate) fn on_run(&mut self, statement: &str, event: Event) {
        if text::may_change_job(statement) {
            self.job_stale = true;
        }
        let short = statement.split_whitespace().collect::<Vec<_>>().join(" ");
        match event
            .result
            .and_then(parley_sql_core::protocol::Answer::into_result::<Outcome>)
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
                self.messages.push(Note::Info(format!(
                    "{short}: {} rows{}",
                    rows.len(),
                    if more { ", more to fetch" } else { "" }
                )));
                self.results
                    .add(statement, ResultView::new(columns, rows, more, cursor));
                self.tab = Tab::Results;
            }
            Ok(Outcome::Count { .. }) if text::cl_command(statement).is_some() => {
                self.note_outcome(statement, gutter::Outcome::Done);
                self.messages.push(Note::Info(format!("{short}: done")));
                if self.queue.is_empty() {
                    self.tab = Tab::Messages;
                }
            }
            Ok(Outcome::Count { count }) => {
                self.note_outcome(statement, gutter::Outcome::Changed(count));
                self.messages
                    .push(Note::Info(format!("{short}: {count} rows changed")));
                if !self.autocommit {
                    self.uncommitted = true;
                }
                if self.queue.is_empty() {
                    self.tab = Tab::Messages;
                }
            }
            Err(e) => {
                self.note_outcome(statement, gutter::Outcome::Failed(e.to_string()));
                // R-SQL-75: where the host says it stopped, marked in the editor.
                self.scripts.mark_error(statement, &e.to_string());
                self.queue.clear();
                self.fail(format!("{short}: {e}"));
                return;
            }
        }
        self.next_statement();
    }

    /// Keeps how `statement` ended for the editor's gutter (R-SQL-62).
    pub(crate) fn note_outcome(&mut self, statement: &str, outcome: gutter::Outcome) {
        let took = self
            .run_started
            .take()
            .map(|t| t.elapsed())
            .unwrap_or_default();
        // R-SQL-89: every run, with how it ended.
        self.ran.push(parley_sql_core::history::Entry::now(
            statement,
            took,
            outcome.clone(),
        ));
        self.scripts.outcomes.note(statement, outcome, took);
    }

    pub(crate) fn fail(&mut self, message: String) {
        self.messages.push(Note::Error(message.clone()));
        self.error_line = Some(message);
    }

    pub(crate) fn back_to_sign_in(&mut self, message: String) {
        self.runner = None;
        self.pending_connect = None;
        // The first sign-on failed: the second does not try the same password.
        self.pending_catalog = None;
        self.database.abandon();
        self.stage = Stage::SignIn;
        self.error_line = Some(message);
    }
}
