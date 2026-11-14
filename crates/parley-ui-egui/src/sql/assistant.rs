//! The SQL assistant in the window (R-SQL-44, R-ADD-3, D25): an Ask row over the editor; the
//! question goes to Claude on a thread of its own, the model's lookups are answered from the
//! schema browser's catalog, and its SQL opens in a new script tab. Nothing runs by itself.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use egui::{TextEdit, Ui};
use parley_core::sql::assistant::{self, Lookup, MAX_ROUNDS, Reply};
use serde_json::Value;

use super::browser::Browser;
use super::database::Database;
use crate::{icons, theme};

/// Where a question is.
#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    /// A request is with Claude.
    Waiting {
        messages: Vec<Value>,
        reply: Receiver<parley_core::Result<Value>>,
        rounds: usize,
    },
    /// The model asked for catalog names; they are being read.
    Looking {
        messages: Vec<Value>,
        content: Value,
        calls: Vec<(String, Lookup)>,
        rounds: usize,
    },
}

/// What the assistant gives the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub sql: String,
    pub explanation: String,
}

/// The assistant of a window.
#[derive(Debug, Default)]
pub struct Assistant {
    /// Whether its add-on is on (R-ADD-3): off, nothing of it shows or is sent.
    pub on: bool,
    pub question: String,
    state: State,
    /// Said under the Ask row: whether it is a problem, and the text.
    note: Option<(bool, String)>,
    /// The key, read when asking (never stored, D25).
    key: String,
}

/// What the Ask row shows about where the question goes.
const DISCLOSURE: &str = "Sends your question and table and column names (never rows) to \
    Anthropic's Claude Opus 5.5, with your ANTHROPIC_API_KEY.";

impl Assistant {
    pub fn busy(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// Sends the question with where it is asked.
    pub fn ask(&mut self, ctx: &egui::Context, release: &str, schema: &str, libraries: &[String]) {
        let question = self.question.trim();
        if question.is_empty() || self.busy() {
            return;
        }
        match assistant::key() {
            Ok(key) => self.key = key,
            Err(e) => {
                self.note = Some((true, e.to_string()));
                return;
            }
        }
        self.note = None;
        let messages = vec![assistant::first_message(
            question, release, schema, libraries,
        )];
        self.send(ctx, messages, 0);
    }

    /// Stops waiting; an answer that comes later is dropped.
    pub fn cancel(&mut self) {
        self.state = State::Idle;
        self.key.clear();
        self.note = Some((false, "Cancelled.".to_owned()));
    }

    fn send(&mut self, ctx: &egui::Context, messages: Vec<Value>, rounds: usize) {
        let body = assistant::request(&messages);
        let key = self.key.clone();
        let wake = ctx.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            // The window may be gone or have cancelled: then nobody wants the reply.
            let _ = tx.send(assistant::send(&key, &body));
            wake.request_repaint();
        });
        self.state = State::Waiting {
            messages,
            reply: rx,
            rounds: rounds + 1,
        };
    }

    /// Moves the question on: a reply read, lookups answered from `browser`, the next request
    /// sent. The answer, once there is one.
    pub fn step(
        &mut self,
        ctx: &egui::Context,
        browser: &mut Browser,
        db: &mut Database,
    ) -> Option<Answer> {
        match std::mem::take(&mut self.state) {
            State::Idle => None,
            State::Waiting {
                messages,
                reply,
                rounds,
            } => match reply.try_recv() {
                Err(TryRecvError::Empty) => {
                    self.state = State::Waiting {
                        messages,
                        reply,
                        rounds,
                    };
                    None
                }
                Err(TryRecvError::Disconnected) => {
                    self.fail("The request stopped without an answer.")
                }
                Ok(Err(e)) => self.fail(&e.to_string()),
                Ok(Ok(value)) => match assistant::read_reply(&value) {
                    Err(e) => self.fail(&e.to_string()),
                    Ok(Reply::Sql { sql, explanation }) => {
                        self.key.clear();
                        self.note = Some((
                            false,
                            "The SQL is in a new script tab: read it before you run it.".to_owned(),
                        ));
                        Some(Answer { sql, explanation })
                    }
                    Ok(Reply::Text(text)) => {
                        self.key.clear();
                        self.note = Some((false, text));
                        None
                    }
                    Ok(Reply::Lookups { content, calls }) if rounds < MAX_ROUNDS => {
                        self.state = State::Looking {
                            messages,
                            content,
                            calls,
                            rounds,
                        };
                        self.step(ctx, browser, db)
                    }
                    Ok(Reply::Lookups { .. }) => {
                        self.fail("Claude looked up too much without answering.")
                    }
                },
            },
            State::Looking {
                mut messages,
                content,
                calls,
                rounds,
            } => {
                let answers: Vec<Option<String>> = calls
                    .iter()
                    .map(|(_, lookup)| browser.tool_answer(lookup, db))
                    .collect();
                if answers.iter().any(Option::is_none) {
                    self.state = State::Looking {
                        messages,
                        content,
                        calls,
                        rounds,
                    };
                    return None;
                }
                let answers: Vec<(String, String)> = calls
                    .into_iter()
                    .zip(answers)
                    .map(|((id, _), text)| (id, text.unwrap_or_default()))
                    .collect();
                messages.extend(assistant::with_answers(&content, &answers));
                self.send(ctx, messages, rounds);
                None
            }
        }
    }

    fn fail(&mut self, why: &str) -> Option<Answer> {
        self.key.clear();
        self.state = State::Idle;
        self.note = Some((true, why.to_owned()));
        None
    }

    /// The Ask row; true when Ask was clicked or Enter pressed in it.
    pub fn row(&mut self, ui: &mut Ui) -> bool {
        let mut asked = false;
        ui.horizontal(|ui| {
            let busy = self.busy();
            let field = TextEdit::singleline(&mut self.question)
                .hint_text("Ask for SQL in plain words, e.g. customers in Århus with open orders");
            let width = (ui.available_width() - 190.0).max(160.0);
            let response = ui.add_enabled(!busy, field.desired_width(width));
            let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let ask = icons::button(icons::LIGHTNING, "Ask");
            if icons::add_named(
                ui,
                !busy && !self.question.trim().is_empty(),
                ask,
                "Ask Claude",
            )
            .on_hover_text(DISCLOSURE)
            .clicked()
                || (entered && !busy)
            {
                asked = true;
            }
            if busy {
                if ui.button("Cancel asking").clicked() {
                    self.cancel();
                }
                ui.spinner();
            }
        });
        match &self.note {
            Some((true, text)) => theme::error(ui, text),
            Some((false, text)) => theme::hint(ui, text),
            None => theme::hint(ui, DISCLOSURE),
        }
        asked
    }
}

/// The script an answer opens as: the explanation as comments, then the statement.
pub fn script(answer: &Answer) -> String {
    let mut out: Vec<String> = answer
        .explanation
        .lines()
        .map(|l| format!("-- {l}").trim_end().to_owned())
        .collect();
    if answer.sql.trim().is_empty() {
        out.push("-- No statement.".to_owned());
    } else {
        out.push(answer.sql.trim().to_owned());
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_opens_as_comments_then_the_statement_r_sql_44() {
        let answer = Answer {
            sql: "SELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'\n".to_owned(),
            explanation: "Kunderne i Århus.\nKun dem med ordrer.".to_owned(),
        };
        assert_eq!(
            script(&answer),
            "-- Kunderne i Århus.\n-- Kun dem med ordrer.\nSELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'"
        );
        let none = Answer {
            sql: String::new(),
            explanation: "Der er ingen ordretabel.".to_owned(),
        };
        assert_eq!(
            script(&none),
            "-- Der er ingen ordretabel.\n-- No statement."
        );
    }

    #[test]
    fn without_a_key_nothing_is_sent_and_the_reason_is_said_r_sql_44() {
        // Only when the test runs without a key in its environment.
        if std::env::var_os(assistant::KEY_VARIABLE).is_some() {
            return;
        }
        let mut a = Assistant {
            question: "kunder".to_owned(),
            ..Assistant::default()
        };
        a.ask(&egui::Context::default(), "V7R3M0", "QGPL", &[]);
        assert!(!a.busy());
        assert!(matches!(&a.note, Some((true, t)) if t.contains("ANTHROPIC_API_KEY")));
    }

    #[test]
    fn a_lookup_waits_for_the_catalog_and_cancel_drops_the_question_r_sql_44() {
        let mut a = Assistant {
            state: State::Looking {
                messages: Vec::new(),
                content: serde_json::json!([]),
                calls: vec![(
                    "t1".to_owned(),
                    Lookup::Tables {
                        library: "QGPL".to_owned(),
                    },
                )],
                rounds: 1,
            },
            ..Assistant::default()
        };
        let mut browser = Browser::default();
        let mut db = Database::default();
        assert_eq!(
            a.step(&egui::Context::default(), &mut browser, &mut db),
            None
        );
        assert!(a.busy(), "still looking while the library is read");
        a.cancel();
        assert!(!a.busy());
    }
}
