//! Anthropic's Messages API (D25): `POST {url}/messages`, `anthropic-version: 2026-10-21`, the
//! server-side refusal fallback (`fallbacks: "default"`, beta `server-side-fallback-2026-07-01`).

use serde_json::{Value, json};

use super::{Call, Reply, SYSTEM, call, reply, text_of, tools};
use crate::{Error, Result};

const VERSION: &str = "2026-10-21";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// The request, without its body.
pub(super) fn request(
    agent: &ureq::Agent,
    url: &str,
    key: Option<&str>,
) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
    agent
        .post(format!("{url}/messages"))
        .header("x-api-key", key.unwrap_or_default())
        .header("anthropic-version", VERSION)
        .header("anthropic-beta", FALLBACK_BETA)
}

/// The body for `messages` so far.
pub(super) fn body(model: &str, messages: &[Value]) -> Value {
    let tools: Vec<Value> = tools()
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "strict": true,
                "input_schema": t.parameters,
            })
        })
        .collect();
    json!({
        "model": model,
        "max_tokens": 16000,
        "fallbacks": "default",
        "output_config": {"effort": "medium"},
        "system": SYSTEM,
        "tools": tools,
        "messages": messages,
    })
}

/// Reads a reply. Thinking, fallback and other blocks stay in the turn, sent back as they came.
pub(super) fn read(value: &Value) -> Result<Reply> {
    let stop = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if stop == "refusal" {
        return Err(Error::Assistant(
            "Claude declined to answer this question.".to_owned(),
        ));
    }
    let content = value.get("content").cloned().unwrap_or_else(|| json!([]));
    let mut calls: Vec<Call> = Vec::new();
    let mut words = Vec::new();
    for block in content.as_array().into_iter().flatten() {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => words.push(text_of(block, "text")),
            Some("tool_use") => calls.push(call(
                text_of(block, "id"),
                block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                block.get("input").unwrap_or(&Value::Null),
            )),
            _ => {}
        }
    }
    let turn = json!({"role": "assistant", "content": content});
    reply(
        turn,
        calls,
        words.join("\n").trim().to_owned(),
        stop == "max_tokens",
    )
}

/// The answers to a turn's lookups, as tool results in one user turn.
pub(super) fn results(answers: &[(String, String)]) -> Value {
    let results: Vec<Value> = answers
        .iter()
        .map(|(id, text)| json!({"type": "tool_result", "tool_use_id": id, "content": text}))
        .collect();
    json!({"role": "user", "content": results})
}

#[cfg(test)]
mod tests {
    use super::super::{AssistantConfig, Chat, Lookup, Provider};
    use super::*;

    #[test]
    fn the_request_names_the_model_tools_and_fallback_r_sql_44() {
        let body = body(
            "claude-opus-5-5",
            &[json!({"role": "user", "content": "q"})],
        );
        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["system"], SYSTEM);
        let names: Vec<&str> = body["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert_eq!(names, ["list_tables", "describe_table", "give_sql"]);
        assert!(
            body.get("temperature").is_none(),
            "no sampling settings on this model"
        );
    }

    #[test]
    fn a_reply_asks_for_lookups_or_answers_r_sql_44() {
        let mut chat = Chat::new(AssistantConfig::default(), "q", "V7R3M0", "QGPL", &[]);
        let reply = json!({
            "stop_reason": "tool_use",
            "content": [
                {"type": "thinking", "thinking": "", "signature": "x"},
                {"type": "tool_use", "id": "t1", "name": "list_tables", "input": {"library": "QGPL"}},
                {"type": "tool_use", "id": "t2", "name": "describe_table", "input": {"library": "QGPL", "table": "KUNDER"}}
            ]
        });
        let Reply::Lookups { turn, calls } = chat.read(&reply).expect("read") else {
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
        chat.answer(turn, &[("t1".to_owned(), "KUNDER (table)".to_owned())]);
        let body = chat.body();
        assert_eq!(
            body["messages"][1]["content"][0]["type"], "thinking",
            "sent back unchanged"
        );
        assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "t1");

        let done = json!({"stop_reason": "tool_use", "content": [
            {"type": "tool_use", "id": "t3", "name": "give_sql",
             "input": {"sql": "SELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'", "explanation": "Kunderne i Århus."}}
        ]});
        assert_eq!(
            chat.read(&done).expect("read"),
            Reply::Sql {
                sql: "SELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'".to_owned(),
                explanation: "Kunderne i Århus.".to_owned(),
            }
        );
        let words = json!({"stop_reason": "end_turn", "content": [{"type": "text", "text": "Hvilken tabel?"}]});
        assert_eq!(
            chat.read(&words).expect("read"),
            Reply::Text("Hvilken tabel?".to_owned())
        );
        assert_eq!(chat.config().provider, Provider::Anthropic);
    }

    #[test]
    fn refusals_and_cut_off_answers_are_errors_r_sql_44() {
        assert!(read(&json!({"stop_reason": "refusal", "content": []})).is_err());
        assert!(read(&json!({"stop_reason": "max_tokens", "content": []})).is_err());
        assert!(read(&json!({})).is_ok(), "an empty reply is no words");
    }

    #[test]
    fn a_tool_use_without_an_id_or_input_is_still_read_not_a_panic_r_sql_44() {
        let reply = json!({"stop_reason": "tool_use", "content": [
            {"type": "tool_use", "name": "describe_table"},
            {"type": "tool_use", "id": 5, "name": "list_tables", "input": "QGPL"},
            "text",
            {"type": 3}
        ]});
        let Ok(Reply::Lookups { calls, .. }) = read(&reply) else {
            panic!("lookups expected");
        };
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            (
                String::new(),
                Lookup::Columns {
                    library: String::new(),
                    table: String::new()
                }
            )
        );
        assert!(read(&json!({"content": "words"})).is_ok());
        assert!(read(&json!([1, 2])).is_ok());
    }
}
