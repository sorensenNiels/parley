//! The SQL assistant in the window (R-SQL-44, R-SQL-48, R-ADD-3, D25, D27): an Ask row over the
//! editor; the question goes to the chosen provider on a thread of its own, the model's lookups
//! are answered from the schema browser's catalog, and its SQL opens in a new script tab.
//! Nothing runs by itself.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use egui::{TextEdit, Ui};
use parley_core::password::Password;
use parley_core::sql::assistant::{
    self, AssistantConfig, Chat, Lookup, MAX_ROUNDS, Provider, Reply,
};
use serde_json::Value;

use super::browser::Browser;
use super::database::Database;
use crate::{icons, theme};

/// The API keys kept in the operating system's keychain, one per provider (R-SQL-48, D27).
/// The launcher gives it; Parley keeps no key in its settings.
pub trait ApiKeys: std::fmt::Debug {
    /// The saved key; `None` when there is none.
    ///
    /// # Errors
    /// When the keychain cannot be read.
    fn get(&self, provider: Provider) -> Result<Option<String>, String>;

    /// Saves `key`, replacing one saved before.
    ///
    /// # Errors
    /// When the keychain cannot be written.
    fn set(&self, provider: Provider, key: &str) -> Result<(), String>;

    /// Removes the saved key; nothing to remove is no error.
    ///
    /// # Errors
    /// When the keychain cannot be written.
    fn delete(&self, provider: Provider) -> Result<(), String>;
}

/// Where a provider's key comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    /// Its environment variable, which comes before the keychain.
    Environment(&'static str),
    Keychain,
    None,
}

/// The key to ask with: the environment variable, then the keychain (D27).
///
/// # Errors
/// When the keychain cannot be read.
pub fn find_key(
    provider: Provider,
    keys: Option<&dyn ApiKeys>,
) -> Result<(KeySource, Option<Password>), String> {
    if let Some(key) = assistant::key_from_environment(provider) {
        let variable = provider.key_variable().unwrap_or_default();
        return Ok((KeySource::Environment(variable), Some(Password::new(key))));
    }
    let saved = match keys {
        Some(keys) => keys.get(provider)?,
        None => None,
    };
    Ok(match saved.filter(|k| !k.trim().is_empty()) {
        Some(key) => (
            KeySource::Keychain,
            Some(Password::new(key.trim().to_owned())),
        ),
        None => (KeySource::None, None),
    })
}

/// Where a question is.
#[derive(Debug, Default)]
enum State {
    #[default]
    Idle,
    /// A request is with the model.
    Waiting {
        chat: Chat,
        reply: Receiver<parley_core::Result<Value>>,
        rounds: usize,
    },
    /// The model asked for catalog names; they are being read.
    Looking {
        chat: Chat,
        turn: Value,
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
    /// Whether it is on (R-ADD-3): off, nothing of it shows or is sent.
    pub on: bool,
    /// The provider, model and address (R-SQL-48).
    pub config: AssistantConfig,
    /// The keychain, if the launcher gave one.
    pub keys: Option<Arc<dyn ApiKeys>>,
    pub question: String,
    state: State,
    /// Said under the Ask row: whether it is a problem, and the text.
    note: Option<(bool, String)>,
    /// The key, found when asking and dropped with the answer (D27).
    key: Option<Password>,
}

/// What the Ask row says about where the question goes.
pub fn disclosure(config: &AssistantConfig) -> String {
    let model = match config.model() {
        "" => String::new(),
        model => format!(" ({model})"),
    };
    format!(
        "Sends your question and table and column names (never rows) to {}{model}.",
        config.provider.receiver()
    )
}

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
        let provider = self.config.provider;
        match find_key(provider, self.keys.as_deref()) {
            Ok((_, None)) if provider.needs_key() => {
                self.note = Some((true, assistant::no_key(provider)));
                return;
            }
            Ok((_, key)) => self.key = key,
            Err(e) => {
                self.note = Some((true, format!("Could not read the keychain: {e}")));
                return;
            }
        }
        self.note = None;
        let chat = Chat::new(self.config.clone(), question, release, schema, libraries);
        self.send(ctx, chat, 0);
    }

    /// Stops waiting; an answer that comes later is dropped.
    pub fn cancel(&mut self) {
        self.state = State::Idle;
        self.key = None;
        self.note = Some((false, "Cancelled.".to_owned()));
    }

    fn send(&mut self, ctx: &egui::Context, chat: Chat, rounds: usize) {
        let body = chat.body();
        let config = chat.config().clone();
        let key = self.key.clone();
        let wake = ctx.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let key = key.as_ref().map(Password::expose);
            // The window may be gone or have cancelled: then nobody wants the reply.
            let _ = tx.send(assistant::send(&config, key, &body));
            wake.request_repaint();
        });
        self.state = State::Waiting {
            chat,
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
                chat,
                reply,
                rounds,
            } => match reply.try_recv() {
                Err(TryRecvError::Empty) => {
                    self.state = State::Waiting {
                        chat,
                        reply,
                        rounds,
                    };
                    None
                }
                Err(TryRecvError::Disconnected) => {
                    self.fail("The request stopped without an answer.")
                }
                Ok(Err(e)) => self.fail(&e.to_string()),
                Ok(Ok(value)) => match chat.read(&value) {
                    Err(e) => self.fail(&e.to_string()),
                    Ok(Reply::Sql { sql, explanation }) => {
                        self.key = None;
                        self.note = Some((
                            false,
                            "The SQL is in a new script tab: read it before you run it.".to_owned(),
                        ));
                        Some(Answer { sql, explanation })
                    }
                    Ok(Reply::Text(text)) => {
                        self.key = None;
                        self.note = Some((false, text));
                        None
                    }
                    Ok(Reply::Lookups { turn, calls }) if rounds < MAX_ROUNDS => {
                        self.state = State::Looking {
                            chat,
                            turn,
                            calls,
                            rounds,
                        };
                        self.step(ctx, browser, db)
                    }
                    Ok(Reply::Lookups { .. }) => {
                        self.fail("The model looked up too much without answering.")
                    }
                },
            },
            State::Looking {
                mut chat,
                turn,
                calls,
                rounds,
            } => {
                let answers: Vec<Option<String>> = calls
                    .iter()
                    .map(|(_, lookup)| browser.tool_answer(lookup, db))
                    .collect();
                if answers.iter().any(Option::is_none) {
                    self.state = State::Looking {
                        chat,
                        turn,
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
                chat.answer(turn, &answers);
                self.send(ctx, chat, rounds);
                None
            }
        }
    }

    fn fail(&mut self, why: &str) -> Option<Answer> {
        self.key = None;
        self.state = State::Idle;
        self.note = Some((true, why.to_owned()));
        None
    }

    /// The Ask row; true when Ask was clicked or Enter pressed in it.
    pub fn row(&mut self, ui: &mut Ui) -> bool {
        let mut asked = false;
        let disclosure = disclosure(&self.config);
        ui.horizontal(|ui| {
            let busy = self.busy();
            let field = TextEdit::singleline(&mut self.question)
                .hint_text("Ask for SQL in plain words, e.g. customers in Århus with open orders");
            let width = (ui.available_width() - 190.0).max(160.0);
            let response = ui.add_enabled(!busy, field.desired_width(width));
            let entered = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let ask = icons::button(icons::LIGHTNING, "Ask");
            if icons::add_named(ui, !busy && !self.question.trim().is_empty(), ask, "Ask")
                .on_hover_text(&disclosure)
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
            None => theme::hint(ui, &disclosure),
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
    use crate::test_support::FakeKeys;

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
    fn without_a_key_nothing_is_sent_and_the_reason_is_said_r_sql_48() {
        // Only when the test runs without a key in its environment.
        if assistant::key_from_environment(Provider::Mistral).is_some() {
            return;
        }
        let mut a = Assistant {
            question: "kunder".to_owned(),
            config: AssistantConfig {
                provider: Provider::Mistral,
                ..AssistantConfig::default()
            },
            keys: Some(Arc::new(FakeKeys::default())),
            ..Assistant::default()
        };
        a.ask(&egui::Context::default(), "V7R3M0", "QGPL", &[]);
        assert!(!a.busy());
        assert!(
            matches!(&a.note, Some((true, t)) if t.contains("MISTRAL_API_KEY") && t.contains("Settings")),
            "{:?}",
            a.note
        );
    }

    #[test]
    fn the_key_comes_from_the_keychain_and_never_shows_in_debug_r_sql_48() {
        if assistant::key_from_environment(Provider::OpenAi).is_some() {
            return;
        }
        let keys = FakeKeys::default();
        assert_eq!(
            find_key(Provider::OpenAi, Some(&keys)).expect("read"),
            (KeySource::None, None)
        );
        keys.set(Provider::OpenAi, " sk-hemmelig ").expect("saved");
        let (source, key) = find_key(Provider::OpenAi, Some(&keys)).expect("read");
        assert_eq!(source, KeySource::Keychain);
        assert_eq!(key.as_ref().map(Password::expose), Some("sk-hemmelig"));
        let a = Assistant {
            key,
            ..Assistant::default()
        };
        assert!(!format!("{a:?}").contains("hemmelig"));
    }

    #[test]
    fn a_local_server_is_asked_without_a_key_r_sql_48() {
        let config = AssistantConfig {
            provider: Provider::LlamaCpp,
            ..AssistantConfig::default()
        };
        assert_eq!(
            disclosure(&config),
            "Sends your question and table and column names (never rows) to your llama-server."
        );
        let mut a = Assistant {
            question: "kunder".to_owned(),
            config,
            ..Assistant::default()
        };
        a.ask(&egui::Context::default(), "V7R3M0", "QGPL", &[]);
        assert!(a.busy(), "sent to llama-server without a key");
        a.cancel();
    }

    #[test]
    fn a_lookup_waits_for_the_catalog_and_cancel_drops_the_question_r_sql_44() {
        let mut a = Assistant {
            state: State::Looking {
                chat: Chat::new(AssistantConfig::default(), "q", "V7R3M0", "QGPL", &[]),
                turn: serde_json::json!({}),
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
