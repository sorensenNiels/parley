//! Other systems in the SQL window (R-SQL-43, D24): saved sessions ticked under "Run on…", each
//! signed on in a helper of its own with a password typed for it and then forgotten (D16). A
//! statement that reads runs on all of them at once; each answers with its first block of rows.

use std::path::Path;

use egui::TextEdit;
use parley_core::SessionConfig;
use parley_core::password::Password;
use parley_core::sql::helper::HelperFiles;
use parley_core::sql::protocol::{Answer, Column, Connect, Connected, Outcome, Request, Row};
use parley_core::sql::runner::{Event, Runner};

use crate::{dialog, theme};

/// How far a system is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Waiting for its user and password.
    SignIn,
    Connecting,
    Ready {
        job: String,
    },
    Failed(String),
}

/// What a system's request in flight was.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Waiting {
    Connect,
    Run(String),
}

/// A system ticked under "Run on…".
#[derive(Debug)]
pub struct System {
    pub name: String,
    config: SessionConfig,
    pub state: State,
    user: String,
    password: String,
    runner: Option<Runner>,
    waiting: Option<(u64, Waiting)>,
}

/// What a system answered, for the window to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// A statement's first rows, and whether it had more.
    Rows {
        system: String,
        statement: String,
        columns: Vec<Column>,
        rows: Vec<Row>,
        more: bool,
    },
    /// A message for Messages: whether it is an error, and its text with the system's name.
    Message(bool, String),
}

/// The systems of a window.
#[derive(Debug, Default)]
pub struct Systems {
    pub list: Vec<System>,
    answers: Vec<Answered>,
}

impl Systems {
    /// Whether `name` is ticked.
    pub fn has(&self, name: &str) -> bool {
        self.list.iter().any(|s| s.name == name)
    }

    /// Ticks `name` (asking for its sign-on), or unticks it (closing its helper).
    pub fn toggle(&mut self, name: &str, config: &SessionConfig, user: &str) {
        if let Some(at) = self.list.iter().position(|s| s.name == name) {
            close(self.list.remove(at));
        } else {
            self.list.push(System {
                name: name.to_owned(),
                config: config.clone(),
                state: State::SignIn,
                user: user.to_owned(),
                password: String::new(),
                runner: None,
                waiting: None,
            });
        }
    }

    /// The systems signed on.
    pub fn ready(&self) -> usize {
        self.list
            .iter()
            .filter(|s| matches!(s.state, State::Ready { .. }))
            .count()
    }

    /// Whether any system has a statement running.
    pub fn busy(&self) -> bool {
        self.list
            .iter()
            .any(|s| matches!(s.waiting, Some((_, Waiting::Run(_)))))
    }

    /// Runs `statement` on every system signed on; it must only read (checked by the window).
    pub fn run(&mut self, statement: &str, max_rows: u32, timeout: u32) {
        for system in &mut self.list {
            if !matches!(system.state, State::Ready { .. }) || system.waiting.is_some() {
                continue;
            }
            if let Some(runner) = &mut system.runner {
                let run = Request::Run {
                    sql: statement.to_owned(),
                    max_rows,
                    timeout,
                    params: Vec::new(),
                };
                system.waiting = Some((runner.send(run), Waiting::Run(statement.to_owned())));
            }
        }
    }

    /// Takes what the systems answered since the last frame.
    pub fn read_events(&mut self) {
        for at in 0..self.list.len() {
            while let Some(event) = self
                .list
                .get(at)
                .and_then(|s| s.runner.as_ref())
                .and_then(Runner::try_event)
            {
                if let Some(system) = self.list.get_mut(at) {
                    let answered = handle(system, event);
                    self.answers.extend(answered);
                }
            }
        }
    }

    /// The answers read since last taken.
    pub fn take_answers(&mut self) -> Vec<Answered> {
        std::mem::take(&mut self.answers)
    }

    /// The sign-on of the first system waiting for one, as a dialog.
    pub fn sign_on_dialog(&mut self, ctx: &egui::Context, java: &Path, files: &HelperFiles) {
        let Some(at) = self.list.iter().position(|s| s.state == State::SignIn) else {
            return;
        };
        let Some(system) = self.list.get_mut(at) else {
            return;
        };
        let title = format!("Sign on to {}", system.name);
        let host = system.config.host.clone();
        let (user, password) = (&mut system.user, &mut system.password);
        let (close, (cancel, go)) =
            dialog::dialog(ctx, "sql-system-sign-on", &title, 420.0, |ui| {
                theme::hint(ui, format!("To run statements that read on {host} too."));
                ui.add_space(6.0);
                egui::Grid::new("sql-system-sign-on")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        let label = ui.label("User");
                        theme::sized_edit(ui, 220.0, TextEdit::singleline(user))
                            .labelled_by(label.id);
                        ui.end_row();
                        let label = ui.label("Password");
                        theme::sized_edit(ui, 220.0, TextEdit::singleline(password).password(true))
                            .labelled_by(label.id);
                        ui.end_row();
                    });
                theme::hint(ui, "Parley keeps no password: it is used once to sign on.");
                let ready = !user.trim().is_empty() && !password.is_empty();
                dialog::actions(ui, theme::primary_button(ui, "Sign on"), ready)
            });
        if close || cancel {
            close_at(&mut self.list, at);
        } else if go {
            let wake = ctx.clone();
            match Runner::start(java, files, move || wake.request_repaint()) {
                Ok(mut runner) => {
                    let connect = Request::Connect(Connect {
                        host: system.config.host.clone(),
                        tls: system.config.tls,
                        user: system.user.trim().to_owned(),
                        password: Password::new(std::mem::take(&mut system.password)),
                        ca_file: system.config.ca_file.clone(),
                    });
                    system.waiting = Some((runner.send(connect), Waiting::Connect));
                    system.runner = Some(runner);
                    system.state = State::Connecting;
                }
                Err(e) => {
                    system.password.clear();
                    system.state = State::Failed(e.to_string());
                }
            }
        }
    }

    /// Ends every system's helper with the window.
    pub fn close_all(&mut self) {
        for system in self.list.drain(..) {
            close(system);
        }
    }
}

fn close_at(list: &mut Vec<System>, at: usize) {
    if at < list.len() {
        close(list.remove(at));
    }
}

fn close(mut system: System) {
    system.password.clear();
    if let Some(mut runner) = system.runner.take() {
        runner.send(Request::Close);
    }
}

/// One answer from a system's helper.
fn handle(system: &mut System, event: Event) -> Option<Answered> {
    let (tag, waiting) = system.waiting.take()?;
    if tag != event.tag && event.tag != 0 {
        system.waiting = Some((tag, waiting));
        return None;
    }
    let name = system.name.clone();
    match waiting {
        Waiting::Connect => match event.result.and_then(Answer::into_result::<Connected>) {
            Ok(connected) => {
                system.state = State::Ready { job: connected.job };
                None
            }
            Err(e) => {
                system.runner = None;
                system.state = State::Failed(e.to_string());
                Some(Answered::Message(
                    true,
                    format!("{name}: not signed on: {e}"),
                ))
            }
        },
        Waiting::Run(statement) => match event.result.and_then(Answer::into_result::<Outcome>) {
            Ok(Outcome::Rows {
                cursor,
                columns,
                rows,
                more,
            }) => {
                // The first block only: a cursor left open would hold the job's resources.
                if more && let Some(runner) = &mut system.runner {
                    runner.send(Request::CloseCursor { cursor });
                }
                Some(Answered::Rows {
                    system: name,
                    statement,
                    columns,
                    rows,
                    more,
                })
            }
            Ok(Outcome::Count { count }) => Some(Answered::Message(
                false,
                format!("{name}: {count} rows changed"),
            )),
            Err(e) => Some(Answered::Message(true, format!("{name}: {e}"))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system(state: State, waiting: Option<(u64, Waiting)>) -> System {
        System {
            name: "Test æøå".to_owned(),
            config: SessionConfig::default(),
            state,
            user: "usr".to_owned(),
            password: String::new(),
            runner: None,
            waiting,
        }
    }

    fn event(tag: u64, json: &str) -> Event {
        Event {
            tag,
            result: Answer::parse(json),
        }
    }

    #[test]
    fn a_system_signs_on_and_its_rows_come_back_with_its_name_r_sql_43() {
        let mut s = system(State::Connecting, Some((1, Waiting::Connect)));
        let none = handle(
            &mut s,
            event(
                1,
                r#"{"id":1,"ok":{"user":"USR","release":"V7R3M0","job":"9/QUSER/QZDASOINIT"}}"#,
            ),
        );
        assert_eq!(none, None);
        assert!(matches!(s.state, State::Ready { .. }));
        s.waiting = Some((2, Waiting::Run("VALUES 1".to_owned())));
        let rows = handle(
            &mut s,
            event(
                2,
                r#"{"id":2,"ok":{"kind":"rows","cursor":5,"columns":[],"rows":[["æøå"]],"more":false}}"#,
            ),
        );
        assert_eq!(
            rows,
            Some(Answered::Rows {
                system: "Test æøå".to_owned(),
                statement: "VALUES 1".to_owned(),
                columns: Vec::new(),
                rows: vec![vec![Some("æøå".to_owned())]],
                more: false,
            })
        );
    }

    #[test]
    fn a_refused_sign_on_and_a_failed_statement_are_said_with_the_name_r_sql_43() {
        let mut s = system(State::Connecting, Some((1, Waiting::Connect)));
        let said = handle(
            &mut s,
            event(
                1,
                r#"{"id":1,"error":{"message":"CPF22E2 æøå","sqlstate":"08004","sqlcode":-1}}"#,
            ),
        );
        assert!(
            matches!(said, Some(Answered::Message(true, ref t)) if t.starts_with("Test æøå: not signed on"))
        );
        assert!(matches!(s.state, State::Failed(_)));
        let mut s = system(
            State::Ready {
                job: "j".to_owned(),
            },
            Some((3, Waiting::Run("SELECT * FROM GONE".to_owned()))),
        );
        let said = handle(
            &mut s,
            event(
                3,
                r#"{"id":3,"error":{"message":"[SQL0204] GONE","sqlstate":"42704","sqlcode":-204}}"#,
            ),
        );
        assert!(
            matches!(said, Some(Answered::Message(true, ref t)) if t.starts_with("Test æøå: "))
        );
        assert!(s.waiting.is_none());
    }

    #[test]
    fn ticking_twice_unticks_and_only_signed_on_systems_count_r_sql_43() {
        let mut systems = Systems::default();
        let config = SessionConfig::default();
        systems.toggle("A", &config, "usr");
        systems.toggle("B", &config, "usr");
        assert!(systems.has("A") && systems.ready() == 0);
        systems.toggle("A", &config, "usr");
        assert!(!systems.has("A") && systems.has("B"));
        systems.list[0].state = State::Ready {
            job: "j".to_owned(),
        };
        assert_eq!(systems.ready(), 1);
        // Without a helper in a test, nothing is sent, and nothing is waited for.
        systems.run("VALUES 1", 100, 0);
        assert!(!systems.busy());
    }
}
