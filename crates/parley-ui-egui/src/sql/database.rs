//! The SQL window's second connection, for the catalog (R-SQL-33, D21): a second helper with a
//! job of its own, so reading the catalog never waits for the operator's statement. It starts
//! with the first, but signs on only once the first has: a mistyped password then counts once
//! against the profile, not twice.

use std::collections::VecDeque;
use std::path::Path;

use parley_core::sql::helper::HelperFiles;
use parley_core::sql::protocol::{Answer, Connected, Outcome, Request, Row};
use parley_core::sql::runner::{Event, Runner};
use parley_core::sql::{catalog, services};

/// Seconds a catalog question may take.
const TIMEOUT: u32 = 60;

/// A question for the catalog: the objects of a library, a table's columns (R-SQL-34), or an
/// object's DDL (R-SQL-35); by system names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    Objects(String),
    Columns {
        library: String,
        table: String,
    },
    /// The parameters of the routines called `name` in `library` (R-SQL-65).
    Parameters {
        library: String,
        name: String,
    },
    /// `title` names the script tab it opens in.
    Ddl {
        library: String,
        name: String,
        kind: catalog::Kind,
        title: String,
    },
    /// A list under the System node (R-SQL-78..80).
    Service(services::View),
    /// A spooled file's text or a job log, for viewer window `viewer` (R-SQL-78, R-SQL-80).
    Text {
        sql: String,
        viewer: u64,
    },
}

impl Ask {
    fn sql(&self) -> String {
        match self {
            Ask::Objects(library) => catalog::objects(library),
            Ask::Columns { library, table } => catalog::columns(library, table),
            Ask::Parameters { library, name } => catalog::parameters(library, name),
            Ask::Ddl {
                library,
                name,
                kind,
                ..
            } => catalog::ddl(library, name, *kind),
            Ask::Service(view) => view.sql(),
            Ask::Text { sql, .. } => sql.clone(),
        }
    }
}

/// What a question got: its rows, or why not.
pub type Answered = (Ask, Result<Vec<Row>, String>);

/// How far the second connection is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Not started, or given up because the first sign-on failed.
    Off,
    /// The helper runs; it waits for the first sign-on.
    Started,
    /// Signing on.
    Connecting,
    /// Signed on as this job.
    Ready { job: String },
    /// It could not start or sign on: why. The window works without it.
    Failed(String),
}

/// The second connection.
#[derive(Debug)]
pub struct Database {
    runner: Option<Runner>,
    pub state: State,
    /// The request in flight: its tag, and its question (none for the sign-on).
    waiting: Option<(u64, Option<Ask>)>,
    /// Questions not yet sent, asked first first.
    queue: VecDeque<Ask>,
    /// Answers not yet taken.
    answers: Vec<Answered>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            runner: None,
            state: State::Off,
            waiting: None,
            queue: VecDeque::new(),
            answers: Vec::new(),
        }
    }
}

impl Database {
    /// Starts its helper beside the first one; it signs on with [`Self::sign_on`].
    pub fn start(&mut self, java: &Path, files: &HelperFiles, ctx: &egui::Context) {
        let wake = ctx.clone();
        match Runner::start(java, files, move || wake.request_repaint()) {
            Ok(runner) => {
                self.runner = Some(runner);
                self.state = State::Started;
            }
            Err(e) => self.state = State::Failed(e.to_string()),
        }
    }

    /// Signs on, once the first connection has with the same `connect`.
    pub fn sign_on(&mut self, connect: Request) {
        if self.state != State::Started {
            return;
        }
        if let Some(runner) = &mut self.runner {
            self.waiting = Some((runner.send(connect), None));
            self.state = State::Connecting;
        }
    }

    /// The first sign-on failed: no second attempt with that password.
    pub fn abandon(&mut self) {
        self.runner = None;
        self.waiting = None;
        self.queue.clear();
        self.state = State::Off;
    }

    /// Asks the catalog; the answer comes from [`Self::take_answers`]. Asked before sign-on, it
    /// waits; asked again while waiting, once.
    pub fn ask(&mut self, ask: Ask) {
        let in_flight = matches!(&self.waiting, Some((_, Some(a))) if *a == ask);
        if !in_flight && !self.queue.contains(&ask) {
            self.queue.push_back(ask);
        }
        self.send_next();
    }

    /// The answers read since last taken.
    pub fn take_answers(&mut self) -> Vec<Answered> {
        std::mem::take(&mut self.answers)
    }

    fn send_next(&mut self) {
        if self.waiting.is_some() || !matches!(self.state, State::Ready { .. }) {
            return;
        }
        let Some(runner) = &mut self.runner else {
            return;
        };
        let Some(ask) = self.queue.pop_front() else {
            return;
        };
        let run = Request::Run {
            sql: ask.sql(),
            // Above every FETCH FIRST in the catalog queries, so no cursor stays open.
            max_rows: 10_000,
            timeout: TIMEOUT,
            params: Vec::new(),
        };
        self.waiting = Some((runner.send(run), Some(ask)));
    }

    /// Takes what its helper answered since the last frame.
    pub fn read_events(&mut self) {
        while let Some(event) = self.runner.as_ref().and_then(Runner::try_event) {
            self.handle(event);
        }
    }

    fn handle(&mut self, event: Event) {
        let Some((tag, ask)) = self.waiting.take() else {
            return;
        };
        if tag != event.tag && event.tag != 0 {
            self.waiting = Some((tag, ask));
            return;
        }
        match ask {
            None => self.signed_on(event),
            Some(ask) => {
                let rows = match event.result.and_then(Answer::into_result::<Outcome>) {
                    Ok(Outcome::Rows { rows, .. }) => Ok(rows),
                    Ok(Outcome::Count { .. }) => Ok(Vec::new()),
                    Err(e) => Err(e.to_string()),
                };
                self.answers.push((ask, rows));
            }
        }
        self.send_next();
    }

    fn signed_on(&mut self, event: Event) {
        match event.result.and_then(Answer::into_result::<Connected>) {
            Ok(connected) if self.state == State::Connecting => {
                self.state = State::Ready { job: connected.job };
            }
            Ok(_) => {}
            Err(e) => {
                self.runner = None;
                self.state = State::Failed(e.to_string());
                // Nothing asked will be answered: say so to whoever waits.
                let why = "No catalog job.".to_owned();
                self.answers
                    .extend(self.queue.drain(..).map(|ask| (ask, Err(why.clone()))));
            }
        }
    }

    /// Ends its helper with the window.
    pub fn close(&mut self) {
        if let Some(mut runner) = self.runner.take() {
            runner.send(Request::Close);
        }
        self.state = State::Off;
    }

    /// Its job's name, once signed on (R-SQL-80: marked among the user's jobs).
    pub fn job(&self) -> Option<&str> {
        match &self.state {
            State::Ready { job } => Some(job),
            State::Off | State::Started | State::Connecting | State::Failed(_) => None,
        }
    }

    /// What the top bar says about it: the text, and whether it is a problem.
    pub fn status(&self) -> Option<(String, bool)> {
        match &self.state {
            State::Off | State::Started => None,
            State::Connecting => Some(("catalog job: signing on…".to_owned(), false)),
            State::Ready { job } => Some((format!("catalog job {job}"), false)),
            State::Failed(why) => Some((format!("No catalog job, so no Explorer: {why}"), true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use parley_core::sql::protocol::Answer;

    use super::*;

    fn answered(d: &mut Database, json: &str) {
        let tag = d.waiting.as_ref().map_or(0, |w| w.0);
        d.handle(Event {
            tag,
            result: Answer::parse(json),
        });
    }

    #[test]
    fn it_signs_on_only_after_the_first_and_names_its_job_r_sql_33() {
        let mut d = Database::default();
        d.sign_on(Request::Hello);
        assert_eq!(d.state, State::Off, "not started: nothing is sent");
        d.state = State::Connecting;
        d.waiting = Some((4, None));
        answered(
            &mut d,
            r#"{"id":4,"ok":{"user":"USR","release":"V7R3M0","job":"123/QUSER/QZDASOINIT"}}"#,
        );
        assert_eq!(
            d.state,
            State::Ready {
                job: "123/QUSER/QZDASOINIT".to_owned()
            }
        );
        assert_eq!(
            d.status(),
            Some(("catalog job 123/QUSER/QZDASOINIT".to_owned(), false))
        );
    }

    #[test]
    fn a_failed_second_sign_on_is_said_and_the_window_goes_on_r_sql_33() {
        let mut d = Database {
            state: State::Connecting,
            waiting: Some((1, None)),
            ..Database::default()
        };
        answered(
            &mut d,
            r#"{"id":1,"error":{"message":"Too many jobs æøå.","sqlstate":"08004","sqlcode":-99999}}"#,
        );
        let (text, problem) = d.status().expect("said");
        assert!(problem && text.contains("Too many jobs æøå."), "{text}");
        d.abandon();
        assert_eq!(d.status(), None);
    }

    #[test]
    fn questions_wait_for_the_sign_on_and_are_answered_in_turn_r_sql_34() {
        let mut d = Database {
            state: State::Ready {
                job: "1/Q/QZDA".to_owned(),
            },
            ..Database::default()
        };
        // No runner in a test, so nothing is sent: both wait in the queue, the second once.
        d.ask(Ask::Objects("QGPL".to_owned()));
        d.ask(Ask::Objects("QGPL".to_owned()));
        d.ask(Ask::Columns {
            library: "QGPL".to_owned(),
            table: "T".to_owned(),
        });
        assert_eq!(d.queue.len(), 2);
        d.waiting = Some((7, d.queue.pop_front()));
        answered(
            &mut d,
            r#"{"id":7,"ok":{"kind":"rows","cursor":0,"columns":[],"rows":[["T","KUNDER","KUNDER","æøå"]],"more":false}}"#,
        );
        let answers = d.take_answers();
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].0, Ask::Objects("QGPL".to_owned()));
        assert_eq!(answers[0].1.as_ref().map(Vec::len), Ok(1));
        assert_eq!(d.take_answers().len(), 0);
    }

    #[test]
    fn a_failed_sign_on_answers_the_questions_waiting_r_sql_34() {
        let mut d = Database {
            state: State::Connecting,
            waiting: Some((1, None)),
            ..Database::default()
        };
        d.ask(Ask::Objects("QGPL".to_owned()));
        answered(
            &mut d,
            r#"{"id":1,"error":{"message":"No.","sqlstate":"08004","sqlcode":-1}}"#,
        );
        let answers = d.take_answers();
        assert!(answers.len() == 1 && answers[0].1.is_err());
        assert!(matches!(d.state, State::Failed(_)));
    }
}
