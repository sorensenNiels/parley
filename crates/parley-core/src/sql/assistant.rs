//! The SQL assistant's talk with Claude (R-SQL-44, D25): Anthropic's Messages API over HTTPS
//! (there is no Rust SDK, so `ureq`, as the update check), with two tools through which the
//! model reads catalog names and one through which it answers. Only names leave the machine:
//! the question, the current schema and library list, and what the tools return.
//!
//! API: `POST https://api.anthropic.com/v1/messages`, `anthropic-version: 2026-10-21`, the
//! server-side refusal fallback (`fallbacks: "default"`, beta `server-side-fallback-2026-07-01`).

use std::time::Duration;

use serde_json::{Value, json};

use crate::{Error, Result};

/// The model (D25).
pub const MODEL: &str = "claude-opus-5-5";
const URL: &str = "https://api.anthropic.com/v1/messages";
const VERSION: &str = "2026-10-21";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// The environment variable the key is read from; Parley never stores it (D16, D25).
pub const KEY_VARIABLE: &str = "ANTHROPIC_API_KEY";
/// A request may take this long: the model thinks before it answers.
const TIMEOUT: Duration = Duration::from_secs(180);
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

/// The key from the environment, or a sentence saying how to give one.
pub fn key() -> Result<String> {
    std::env::var(KEY_VARIABLE)
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            Error::Assistant(format!(
                "The SQL assistant needs your Anthropic API key in the environment variable \
                 {KEY_VARIABLE}; set it and start Parley again."
            ))
        })
}

/// The tools the model may call.
fn tools() -> Value {
    let strict_object = |properties: Value, required: &[&str]| {
        json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        })
    };
    json!([
        {
            "name": "list_tables",
            "description": "The tables, views, procedures and functions in an IBM i library: \
                            name, kind and text. Use the library's system name.",
            "strict": true,
            "input_schema": strict_object(
                json!({"library": {"type": "string", "description": "The library, e.g. QGPL"}}),
                &["library"],
            ),
        },
        {
            "name": "describe_table",
            "description": "The columns of a table or view: name, type, whether null is \
                            allowed, heading and text.",
            "strict": true,
            "input_schema": strict_object(
                json!({
                    "library": {"type": "string"},
                    "table": {"type": "string", "description": "The table's SQL name"},
                }),
                &["library", "table"],
            ),
        },
        {
            "name": "give_sql",
            "description": "The answer: one SQL statement and a short explanation.",
            "strict": true,
            "input_schema": strict_object(
                json!({"sql": {"type": "string"}, "explanation": {"type": "string"}}),
                &["sql", "explanation"],
            ),
        },
    ])
}

/// The first message: the question and where it is asked.
pub fn first_message(question: &str, release: &str, schema: &str, libraries: &[String]) -> Value {
    json!({
        "role": "user",
        "content": format!(
            "IBM i release: {release}\nCurrent schema: {schema}\nLibrary list: {}\n\nQuestion: {question}",
            libraries.join(", ")
        ),
    })
}

/// The request body for `messages` so far.
pub fn request(messages: &[Value]) -> Value {
    json!({
        "model": MODEL,
        "max_tokens": 16000,
        "fallbacks": "default",
        "output_config": {"effort": "medium"},
        "system": SYSTEM,
        "tools": tools(),
        "messages": messages,
    })
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
    /// Look these up and send the answers back, each with its tool call's id; `content` is
    /// the reply's content, sent back unchanged with them.
    Lookups {
        content: Value,
        calls: Vec<(String, Lookup)>,
    },
    /// The answer.
    Sql { sql: String, explanation: String },
    /// The model answered in words only.
    Text(String),
}

fn text_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Reads a reply of the Messages API.
pub fn read_reply(reply: &Value) -> Result<Reply> {
    let stop = reply
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if stop == "refusal" {
        return Err(Error::Assistant(
            "Claude declined to answer this question.".to_owned(),
        ));
    }
    let content = reply.get("content").cloned().unwrap_or_else(|| json!([]));
    let blocks = content.as_array().cloned().unwrap_or_default();
    let mut calls = Vec::new();
    let mut words = Vec::new();
    for block in &blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => words.push(text_of(block, "text")),
            Some("tool_use") => {
                let id = text_of(block, "id");
                let input = block.get("input").cloned().unwrap_or_default();
                match block.get("name").and_then(Value::as_str) {
                    Some("give_sql") => {
                        return Ok(Reply::Sql {
                            sql: text_of(&input, "sql"),
                            explanation: text_of(&input, "explanation"),
                        });
                    }
                    Some("list_tables") => calls.push((
                        id,
                        Lookup::Tables {
                            library: text_of(&input, "library"),
                        },
                    )),
                    Some("describe_table") => calls.push((
                        id,
                        Lookup::Columns {
                            library: text_of(&input, "library"),
                            table: text_of(&input, "table"),
                        },
                    )),
                    // An unknown tool: answered as an error, so the model goes on.
                    _ => calls.push((
                        id,
                        Lookup::Tables {
                            library: String::new(),
                        },
                    )),
                }
            }
            // Thinking, fallback and other blocks are sent back as they came.
            _ => {}
        }
    }
    if calls.is_empty() {
        let text = words.join("\n").trim().to_owned();
        if stop == "max_tokens" && text.is_empty() {
            return Err(Error::Assistant(
                "The answer was cut off before it was done.".to_owned(),
            ));
        }
        return Ok(Reply::Text(text));
    }
    Ok(Reply::Lookups { content, calls })
}

/// The next messages after a reply's lookups: the reply as the assistant's turn, then the
/// answers as tool results (all in one user turn).
pub fn with_answers(content: &Value, answers: &[(String, String)]) -> [Value; 2] {
    let results: Vec<Value> = answers
        .iter()
        .map(|(id, text)| json!({"type": "tool_result", "tool_use_id": id, "content": text}))
        .collect();
    [
        json!({"role": "assistant", "content": content}),
        json!({"role": "user", "content": results}),
    ]
}

/// Sends `body` with `key` and returns the reply. Blocks for up to three minutes: run it off
/// the UI thread.
pub fn send(key: &str, body: &Value) -> Result<Value> {
    let tls = ureq::tls::TlsConfig::builder()
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .tls_config(tls)
        .build()
        .into();
    let failed = |e: ureq::Error| Error::Assistant(format!("Could not reach Anthropic: {e}"));
    let mut response = agent
        .post(URL)
        .header("x-api-key", key)
        .header("anthropic-version", VERSION)
        .header("anthropic-beta", FALLBACK_BETA)
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
    answer(status, &bytes)
}

/// A reply's body by its HTTP status: the message, or the API's error said plainly.
pub fn answer(status: u16, body: &[u8]) -> Result<Value> {
    let value: Value = serde_json::from_slice(body).unwrap_or_default();
    if (200..300).contains(&status) {
        return Ok(value);
    }
    let message = value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("no message");
    let why = match status {
        401 => "the API key was refused",
        429 => "too many requests just now; try again in a moment",
        529 | 503 => "Anthropic is busy; try again in a moment",
        _ => "the request failed",
    };
    Err(Error::Assistant(format!(
        "Claude: {why} (HTTP {status}: {message})."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_names_the_model_tools_and_fallback_r_sql_44() {
        let messages = [first_message(
            "Kunder i Århus?",
            "V7R3M0",
            "NODEJT400",
            &["QGPL".to_owned()],
        )];
        let body = request(&messages);
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["fallbacks"], "default");
        let names: Vec<&str> = body["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert_eq!(names, ["list_tables", "describe_table", "give_sql"]);
        let first = body["messages"][0]["content"].as_str().expect("text");
        assert!(
            first.contains("Current schema: NODEJT400")
                && first.ends_with("Question: Kunder i Århus?")
        );
        assert!(
            body.get("temperature").is_none(),
            "no sampling settings on this model"
        );
    }

    #[test]
    fn a_reply_asks_for_lookups_or_answers_r_sql_44() {
        let reply = json!({
            "stop_reason": "tool_use",
            "content": [
                {"type": "thinking", "thinking": "", "signature": "x"},
                {"type": "tool_use", "id": "t1", "name": "list_tables", "input": {"library": "QGPL"}},
                {"type": "tool_use", "id": "t2", "name": "describe_table", "input": {"library": "QGPL", "table": "KUNDER"}}
            ]
        });
        let Reply::Lookups { content, calls } = read_reply(&reply).expect("read") else {
            panic!("lookups expected");
        };
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[1].1,
            Lookup::Columns {
                library: "QGPL".to_owned(),
                table: "KUNDER".to_owned()
            }
        );
        let [assistant, user] =
            with_answers(&content, &[("t1".to_owned(), "KUNDER (table)".to_owned())]);
        assert_eq!(
            assistant["content"][0]["type"], "thinking",
            "sent back unchanged"
        );
        assert_eq!(user["content"][0]["tool_use_id"], "t1");

        let done = json!({"stop_reason": "tool_use", "content": [
            {"type": "tool_use", "id": "t3", "name": "give_sql",
             "input": {"sql": "SELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'", "explanation": "Kunderne i Århus."}}
        ]});
        assert_eq!(
            read_reply(&done).expect("read"),
            Reply::Sql {
                sql: "SELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'".to_owned(),
                explanation: "Kunderne i Århus.".to_owned(),
            }
        );
        let words = json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": "Hvilken tabel?"}]});
        assert_eq!(
            read_reply(&words).expect("read"),
            Reply::Text("Hvilken tabel?".to_owned())
        );
    }

    #[test]
    fn refusals_cut_off_answers_and_http_errors_are_said_plainly_r_sql_44() {
        let refused = json!({"stop_reason": "refusal", "content": []});
        assert!(read_reply(&refused).is_err());
        let cut = json!({"stop_reason": "max_tokens", "content": []});
        assert!(read_reply(&cut).is_err());
        let error = br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let said = answer(401, error).expect_err("refused").to_string();
        assert!(
            said.contains("API key was refused") && said.contains("invalid x-api-key"),
            "{said}"
        );
        assert!(answer(500, b"not json").is_err());
        assert_eq!(answer(200, br#"{"id":"m"}"#).expect("ok")["id"], "m");
    }
}
