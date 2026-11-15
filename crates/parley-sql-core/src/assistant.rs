//! The SQL assistant's talk with a language model (R-SQL-44, R-SQL-48, D25, D27): the model
//! reads catalog names through two tools and answers through a third. Only names leave the
//! machine: the question, the current schema and library list, and what the tools return.
//!
//! Two wire formats cover the providers: Anthropic's Messages API ([`anthropic`]) and the
//! OpenAI Chat Completions format ([`openai`]) that OpenAI, Google Gemini, Mistral, llama.cpp's
//! `llama-server` and most other servers speak. Both go over HTTP(S) with `ureq`, as the update
//! check does. A [`Chat`] keeps the conversation in its provider's format.

mod anthropic;
mod openai;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Error, Result};

/// A request may take this long: models think before they answer, and a local one is slow.
const TIMEOUT: Duration = Duration::from_secs(300);
/// The biggest answer read.
const MAX_ANSWER: u64 = 4 * 1024 * 1024;
/// Rounds of tool calls before the assistant gives up.
pub const MAX_ROUNDS: usize = 10;

const SYSTEM: &str = "You write SQL for Db2 for IBM i. The user asks in plain words; answer with \
one statement for the IBM i release given. Use only libraries, tables and columns you have \
seen through list_tables and describe_table: look them up first, and never invent a name. \
Qualify tables with their library (LIB.TABLE). Prefer statements that read; if the user asks \
for a change, write it, and say in the explanation what it changes. Finish by calling \
give_sql once, with the statement and a short explanation of two or three sentences. If the \
question cannot be answered from the catalog, call give_sql with an empty statement and say why.";

/// Who answers the questions (R-SQL-48, D27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    #[default]
    Anthropic,
    OpenAi,
    Gemini,
    Mistral,
    /// llama.cpp's `llama-server`, run by the operator (D27).
    LlamaCpp,
    /// Any other server that speaks the OpenAI format, at an address the operator gives.
    Custom,
}

/// How a provider is spoken to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Anthropic,
    OpenAi,
}

impl Provider {
    pub const ALL: [Self; 6] = [
        Self::Anthropic,
        Self::OpenAi,
        Self::Gemini,
        Self::Mistral,
        Self::LlamaCpp,
        Self::Custom,
    ];

    /// Its name in the settings.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic (Claude)",
            Self::OpenAi => "OpenAI",
            Self::Gemini => "Google Gemini",
            Self::Mistral => "Mistral",
            Self::LlamaCpp => "llama.cpp (local)",
            Self::Custom => "OpenAI-compatible",
        }
    }

    /// Who receives what is sent, in "sends … to {who}".
    pub const fn receiver(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
            Self::Gemini => "Google",
            Self::Mistral => "Mistral",
            Self::LlamaCpp => "your llama-server",
            Self::Custom => "the server you named",
        }
    }

    /// The key it is filed under in the keychain and the settings file.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "open-ai",
            Self::Gemini => "gemini",
            Self::Mistral => "mistral",
            Self::LlamaCpp => "llama-cpp",
            Self::Custom => "custom",
        }
    }

    const fn format(self) -> Format {
        match self {
            Self::Anthropic => Format::Anthropic,
            Self::OpenAi | Self::Gemini | Self::Mistral | Self::LlamaCpp | Self::Custom => {
                Format::OpenAi
            }
        }
    }

    /// The address used when none is given; empty when one must be given.
    pub const fn default_url(self) -> &'static str {
        match self {
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Self::Mistral => "https://api.mistral.ai/v1",
            Self::LlamaCpp => "http://localhost:8080/v1",
            Self::Custom => "",
        }
    }

    /// The model used when none is given; empty for the server's own (llama-server answers
    /// with the model it was started with).
    pub const fn default_model(self) -> &'static str {
        match self {
            Self::Anthropic => "claude-opus-5-5",
            Self::OpenAi => "gpt-5",
            Self::Gemini => "gemini-2.5-pro",
            Self::Mistral => "mistral-large-latest",
            Self::LlamaCpp | Self::Custom => "",
        }
    }

    /// The environment variable a key is read from before the keychain.
    pub const fn key_variable(self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            Self::OpenAi => Some("OPENAI_API_KEY"),
            Self::Gemini => Some("GEMINI_API_KEY"),
            Self::Mistral => Some("MISTRAL_API_KEY"),
            Self::LlamaCpp | Self::Custom => None,
        }
    }

    /// Whether it refuses requests without a key; a local server may do without one.
    pub const fn needs_key(self) -> bool {
        match self {
            Self::Anthropic | Self::OpenAi | Self::Gemini | Self::Mistral => true,
            Self::LlamaCpp | Self::Custom => false,
        }
    }

    /// Whether every reply must be a tool call (`tool_choice: "required"`). The assistant always
    /// ends with `give_sql`, so nothing is lost; small local models otherwise write their calls
    /// as text, which `llama-server` does not read as calls (R-SQL-48, seen with
    /// Qwen2.5-Coder-7B). Only where tried: the cloud providers are left to choose.
    pub const fn forces_tool_calls(self) -> bool {
        match self {
            Self::LlamaCpp => true,
            Self::Anthropic | Self::OpenAi | Self::Gemini | Self::Mistral | Self::Custom => false,
        }
    }

    /// Whether the address can be changed in the settings.
    pub const fn has_url(self) -> bool {
        match self {
            Self::LlamaCpp | Self::Custom => true,
            Self::Anthropic | Self::OpenAi | Self::Gemini | Self::Mistral => false,
        }
    }
}

/// The assistant's settings: the provider, and the model and address when not its defaults.
/// Never a key (D27): that is in the keychain or the environment.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssistantConfig {
    pub provider: Provider,
    /// Empty for the provider's default.
    pub model: String,
    /// The server's base address (up to `/v1`); empty for the provider's default.
    pub url: String,
}

impl AssistantConfig {
    /// The model asked for.
    pub fn model(&self) -> &str {
        match self.model.trim() {
            "" => self.provider.default_model(),
            model => model,
        }
    }

    /// The base address, without a trailing `/`.
    pub fn url(&self) -> &str {
        let given = self.url.trim();
        if self.provider.has_url() && !given.is_empty() {
            given.trim_end_matches('/')
        } else {
            self.provider.default_url()
        }
    }
}

/// The key from the provider's environment variable, if it is set.
pub fn key_from_environment(provider: Provider) -> Option<String> {
    let variable = provider.key_variable()?;
    std::env::var(variable)
        .ok()
        .map(|k| k.trim().to_owned())
        .filter(|k| !k.is_empty())
}

/// A tool the model may call, in neither format.
struct Tool {
    name: &'static str,
    description: &'static str,
    /// A JSON schema of an object, every property required, no others.
    parameters: Value,
}

fn object(properties: &Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn tools() -> [Tool; 3] {
    [
        Tool {
            name: "list_tables",
            description: "The tables, views, procedures and functions in an IBM i library: \
                          name, kind and text. Use the library's system name.",
            parameters: object(
                &json!({"library": {"type": "string", "description": "The library, e.g. QGPL"}}),
                &["library"],
            ),
        },
        Tool {
            name: "describe_table",
            description: "The columns of a table or view: name, type, whether null is \
                          allowed, heading and text.",
            parameters: object(
                &json!({
                    "library": {"type": "string"},
                    "table": {"type": "string", "description": "The table's SQL name"},
                }),
                &["library", "table"],
            ),
        },
        Tool {
            name: "give_sql",
            description: "The answer: one SQL statement and a short explanation.",
            parameters: object(
                &json!({"sql": {"type": "string"}, "explanation": {"type": "string"}}),
                &["sql", "explanation"],
            ),
        },
    ]
}

/// A catalog tool the model called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Tables { library: String },
    Columns { library: String, table: String },
}

/// What a reply asks of Parley.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// Look these up and give [`Chat::answer`] the answers, each with its call's id; `turn`
    /// is the model's turn, kept in the conversation with them.
    Lookups {
        turn: Value,
        calls: Vec<(String, Lookup)>,
    },
    /// The answer.
    Sql { sql: String, explanation: String },
    /// The model answered in words only.
    Text(String),
}

/// One tool call, read in either format.
enum Call {
    Lookup(String, Lookup),
    Answer { sql: String, explanation: String },
}

fn text_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// A call of tool `name` with `input`.
fn call(id: String, name: &str, input: &Value) -> Call {
    match name {
        "give_sql" => Call::Answer {
            sql: text_of(input, "sql"),
            explanation: text_of(input, "explanation"),
        },
        "describe_table" => Call::Lookup(
            id,
            Lookup::Columns {
                library: text_of(input, "library"),
                table: text_of(input, "table"),
            },
        ),
        // list_tables, or an unknown tool: an empty library is answered as an error, so the
        // model goes on.
        "list_tables" => Call::Lookup(
            id,
            Lookup::Tables {
                library: text_of(input, "library"),
            },
        ),
        _ => Call::Lookup(
            id,
            Lookup::Tables {
                library: String::new(),
            },
        ),
    }
}

/// A reply from its calls and words: the answer, the lookups, or the words.
fn reply(turn: Value, calls: Vec<Call>, words: String, cut_off: bool) -> Result<Reply> {
    let mut lookups = Vec::new();
    for call in calls {
        match call {
            Call::Answer { sql, explanation } => return Ok(Reply::Sql { sql, explanation }),
            Call::Lookup(id, lookup) => lookups.push((id, lookup)),
        }
    }
    if !lookups.is_empty() {
        return Ok(Reply::Lookups {
            turn,
            calls: lookups,
        });
    }
    if cut_off && words.is_empty() {
        return Err(Error::Assistant(
            "The answer was cut off before it was done.".to_owned(),
        ));
    }
    Ok(Reply::Text(words))
}

/// A question and the turns since, in the provider's format.
#[derive(Debug, Clone)]
pub struct Chat {
    config: AssistantConfig,
    messages: Vec<Value>,
}

impl Chat {
    /// Starts with the question and where it is asked.
    pub fn new(
        config: AssistantConfig,
        question: &str,
        release: &str,
        schema: &str,
        libraries: &[String],
    ) -> Self {
        let text = format!(
            "IBM i release: {release}\nCurrent schema: {schema}\nLibrary list: {}\n\nQuestion: {question}",
            libraries.join(", ")
        );
        Self {
            config,
            messages: vec![json!({"role": "user", "content": text})],
        }
    }

    pub const fn config(&self) -> &AssistantConfig {
        &self.config
    }

    /// The request body for the conversation so far.
    pub fn body(&self) -> Value {
        match self.config.provider.format() {
            Format::Anthropic => anthropic::body(self.config.model(), &self.messages),
            Format::OpenAi => {
                openai::body(self.config.provider, self.config.model(), &self.messages)
            }
        }
    }

    /// Reads a reply.
    pub fn read(&self, reply: &Value) -> Result<Reply> {
        match self.config.provider.format() {
            Format::Anthropic => anthropic::read(reply),
            Format::OpenAi => openai::read(reply, self.config.provider),
        }
    }

    /// Adds the model's `turn` and the answers to its lookups.
    pub fn answer(&mut self, turn: Value, answers: &[(String, String)]) {
        self.messages.push(turn);
        match self.config.provider.format() {
            Format::Anthropic => self.messages.push(anthropic::results(answers)),
            Format::OpenAi => self.messages.extend(openai::results(answers)),
        }
    }
}

/// Sends `body` to the provider and returns its reply. Blocks for up to five minutes: run it
/// off the UI thread.
pub fn send(config: &AssistantConfig, key: Option<&str>, body: &Value) -> Result<Value> {
    let provider = config.provider;
    if config.url().is_empty() {
        return Err(Error::Assistant(
            "Give the server's address in Settings ▸ SQL assistant.".to_owned(),
        ));
    }
    if provider.needs_key() && key.is_none_or(str::is_empty) {
        return Err(Error::Assistant(no_key(provider)));
    }
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .tls_config(tls)
        .build()
        .into();
    let failed = |e: ureq::Error| {
        Error::Assistant(format!(
            "Could not reach {} at {}: {e}",
            provider.receiver(),
            config.url()
        ))
    };
    let key = key.filter(|k| !k.is_empty());
    let request = match provider.format() {
        Format::Anthropic => anthropic::request(&agent, config.url(), key),
        Format::OpenAi => openai::request(&agent, config.url(), key),
    };
    let mut response = request
        .header("content-type", "application/json")
        .send(body.to_string())
        .map_err(failed)?;
    let status = response.status().as_u16();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_ANSWER)
        .read_to_vec()
        .map_err(failed)?;
    answer(provider, status, &bytes)
}

/// What to say when a provider that needs a key has none.
pub fn no_key(provider: Provider) -> String {
    let variable = provider
        .key_variable()
        .map(|v| format!(" or set {v}"))
        .unwrap_or_default();
    format!(
        "The SQL assistant needs your {} API key: save it in Settings ▸ SQL assistant{variable}.",
        provider.receiver()
    )
}

/// A reply's body by its HTTP status: the message, or the API's error said plainly.
pub fn answer(provider: Provider, status: u16, body: &[u8]) -> Result<Value> {
    let parsed: std::result::Result<Value, _> = serde_json::from_slice(body);
    if (200..300).contains(&status) {
        // A proxy's or a wrong address's web page is not an answer: say so, not a blank one.
        return parsed.map_err(|_| {
            Error::Assistant(format!(
                "{}: the answer was not JSON; check the address in Settings ▸ SQL assistant.",
                provider.label()
            ))
        });
    }
    // An error body that is not JSON says nothing more than its status.
    let value = parsed.unwrap_or_default();
    // Most servers say `{"error": {"message": …}}`; Gemini wraps it in a list.
    let message = ["/error/message", "/0/error/message", "/message"]
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_str))
        .unwrap_or("no message");
    let why = match status {
        401 | 403 => "the API key was refused",
        404 => "no such model or address",
        429 => "too many requests just now; try again in a moment",
        529 | 503 => "busy; try again in a moment",
        _ => "the request failed",
    };
    Err(Error::Assistant(format!(
        "{}: {why} (HTTP {status}: {message}).",
        provider.label()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(provider: Provider) -> Chat {
        let config = AssistantConfig {
            provider,
            ..AssistantConfig::default()
        };
        Chat::new(
            config,
            "Kunder i Århus?",
            "V7R3M0",
            "NODEJT400",
            &["QGPL".to_owned()],
        )
    }

    #[test]
    fn each_provider_has_an_address_and_a_model_unless_the_operator_gives_them_r_sql_48() {
        for provider in Provider::ALL {
            let config = AssistantConfig {
                provider,
                ..AssistantConfig::default()
            };
            assert_eq!(
                config.url().is_empty(),
                provider == Provider::Custom,
                "{provider:?}"
            );
            assert_eq!(provider.needs_key(), provider.key_variable().is_some());
        }
        let local = AssistantConfig {
            provider: Provider::LlamaCpp,
            model: " ".to_owned(),
            url: "http://gpu-box:8080/v1/".to_owned(),
        };
        assert_eq!(local.url(), "http://gpu-box:8080/v1");
        assert_eq!(local.model(), "");
        let fixed = AssistantConfig {
            provider: Provider::OpenAi,
            url: "http://elsewhere".to_owned(),
            ..AssistantConfig::default()
        };
        assert_eq!(fixed.url(), "https://api.openai.com/v1", "not changeable");
    }

    #[test]
    fn the_settings_never_hold_a_key_and_read_old_files_r_sql_48() {
        let json = serde_json::to_string(&AssistantConfig::default()).expect("serializes");
        assert!(!json.contains("key"), "{json}");
        let old: AssistantConfig = serde_json::from_str("{}").expect("defaults");
        assert_eq!(old.provider, Provider::Anthropic);
        let named: AssistantConfig =
            serde_json::from_str(r#"{"provider":"llama-cpp"}"#).expect("by id");
        assert_eq!(named.provider, Provider::LlamaCpp);
    }

    #[test]
    fn without_a_key_or_an_address_nothing_is_sent_r_sql_48() {
        let config = AssistantConfig {
            provider: Provider::Gemini,
            ..AssistantConfig::default()
        };
        let said = send(&config, None, &json!({}))
            .expect_err("no key")
            .to_string();
        assert!(said.contains("GEMINI_API_KEY"), "{said}");
        let custom = AssistantConfig {
            provider: Provider::Custom,
            ..AssistantConfig::default()
        };
        let said = send(&custom, None, &json!({}))
            .expect_err("no address")
            .to_string();
        assert!(said.contains("address"), "{said}");
    }

    #[test]
    fn http_errors_are_said_plainly_by_provider_r_sql_44() {
        let error = br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let said = answer(Provider::Anthropic, 401, error)
            .expect_err("refused")
            .to_string();
        assert!(
            said.contains("API key was refused") && said.contains("invalid x-api-key"),
            "{said}"
        );
        let gemini = br#"[{"error":{"code":400,"message":"API key not valid."}}]"#;
        let said = answer(Provider::Gemini, 400, gemini)
            .expect_err("bad")
            .to_string();
        assert!(said.contains("API key not valid."), "{said}");
        assert!(answer(Provider::OpenAi, 500, b"not json").is_err());
        assert_eq!(
            answer(Provider::OpenAi, 200, br#"{"id":"m"}"#).expect("ok")["id"],
            "m"
        );
    }

    #[test]
    fn a_successful_reply_that_is_not_json_is_an_error_not_a_blank_answer_r_sql_44() {
        for body in [&b"<html>Proxy sign-in</html>"[..], b"", b"{\"cut"] {
            let said = answer(Provider::LlamaCpp, 200, body)
                .expect_err("not an answer")
                .to_string();
            assert!(said.contains("not JSON"), "{said}");
        }
    }

    #[test]
    fn the_first_message_says_where_the_question_is_asked_r_sql_44() {
        for provider in [Provider::Anthropic, Provider::Mistral] {
            let body = chat(provider).body();
            let first = body["messages"]
                .as_array()
                .expect("messages")
                .iter()
                .find(|m| m["role"] == "user")
                .and_then(|m| m["content"].as_str())
                .expect("the question")
                .to_owned();
            assert!(
                first.contains("Current schema: NODEJT400")
                    && first.ends_with("Question: Kunder i Århus?"),
                "{first}"
            );
        }
    }
}
