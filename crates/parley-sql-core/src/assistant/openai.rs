//! The OpenAI Chat Completions format (D27): `POST {url}/chat/completions` with a bearer key,
//! spoken by OpenAI, Google Gemini's OpenAI endpoint, Mistral, llama.cpp's `llama-server`
//! (started with `--jinja` for tool calls) and most other servers. Only what all of them
//! accept is sent: no `strict`, no token limit (OpenAI's newer models refuse `max_tokens`).

use serde_json::{Value, json};

use super::{Call, Provider, Reply, SYSTEM, call, reply, tools};
use crate::{Error, Result};

/// The request, without its body.
pub(super) fn request(
    agent: &ureq::Agent,
    url: &str,
    key: Option<&str>,
) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
    let request = agent.post(format!("{url}/chat/completions"));
    match key {
        Some(key) => request.header("authorization", format!("Bearer {key}")),
        None => request,
    }
}

/// The body for `messages` so far; without a model, the server's own.
pub(super) fn body(provider: Provider, model: &str, messages: &[Value]) -> Value {
    let tools: Vec<Value> = tools()
        .into_iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                },
            })
        })
        .collect();
    let mut all = vec![json!({"role": "system", "content": SYSTEM})];
    all.extend_from_slice(messages);
    let mut body = json!({"messages": all, "tools": tools});
    if provider.forces_tool_calls() {
        body["tool_choice"] = json!("required");
    }
    if !model.is_empty() {
        body["model"] = json!(model);
    }
    body
}

/// Reads a reply: the first choice's message.
pub(super) fn read(value: &Value, provider: Provider) -> Result<Reply> {
    let choice = value.pointer("/choices/0").unwrap_or(&Value::Null);
    let finish = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message = choice.get("message").unwrap_or(&Value::Null);
    let refused = message
        .get("refusal")
        .and_then(Value::as_str)
        .is_some_and(|r| !r.trim().is_empty());
    if refused || finish == "content_filter" {
        return Err(Error::Assistant(format!(
            "{} declined to answer this question.",
            provider.label()
        )));
    }
    let words = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let tool_calls: Vec<Value> = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut kept = Vec::new();
    let mut calls: Vec<Call> = Vec::new();
    for (i, tool_call) in tool_calls.into_iter().enumerate() {
        let mut tool_call = tool_call;
        // A call that is not an object (a malformed reply) names nothing to look up: skip it
        // rather than index into it, which would panic.
        let Some(object) = tool_call.as_object_mut() else {
            continue;
        };
        // Every call is answered by its id: a server that sends none gets one.
        let id = match object.get("id").and_then(Value::as_str) {
            Some(id) if !id.is_empty() => id.to_owned(),
            _ => {
                let id = format!("call{i}");
                object.insert("id".to_owned(), json!(id));
                id
            }
        };
        let function = tool_call.get("function").unwrap_or(&Value::Null);
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        calls.push(call(id, name, &arguments(function)));
        kept.push(tool_call);
    }
    let mut turn = json!({"role": "assistant", "content": words});
    if !kept.is_empty() {
        turn["tool_calls"] = Value::Array(kept);
    }
    reply(turn, calls, words, finish == "length")
}

/// A call's arguments: a JSON text by the format, an object from some local servers.
fn arguments(function: &Value) -> Value {
    match function.get("arguments") {
        Some(Value::String(text)) => serde_json::from_str(text).unwrap_or_default(),
        Some(object) => object.clone(),
        None => Value::Null,
    }
}

/// The answers to a turn's lookups: a tool message each.
pub(super) fn results(answers: &[(String, String)]) -> Vec<Value> {
    answers
        .iter()
        .map(|(id, text)| json!({"role": "tool", "tool_call_id": id, "content": text}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{AssistantConfig, Chat, Lookup};
    use super::*;

    fn chat(provider: Provider) -> Chat {
        let config = AssistantConfig {
            provider,
            ..AssistantConfig::default()
        };
        Chat::new(config, "Kunder i Århus?", "V7R3M0", "QGPL", &[])
    }

    #[test]
    fn the_request_has_the_system_prompt_tools_and_model_r_sql_48() {
        let body = chat(Provider::OpenAi).body();
        assert_eq!(body["model"], "gpt-5");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        let names: Vec<&str> = body["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert_eq!(names, ["list_tables", "describe_table", "give_sql"]);
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("tool_choice").is_none(), "OpenAI chooses");
        let local = chat(Provider::LlamaCpp).body();
        assert!(local.get("model").is_none(), "llama-server's own model");
        assert_eq!(
            local["tool_choice"], "required",
            "calls, not calls written as text"
        );
    }

    #[test]
    fn tool_calls_are_read_and_answered_by_id_r_sql_48() {
        let mut chat = chat(Provider::Mistral);
        let reply = json!({"choices": [{"finish_reason": "tool_calls", "message": {
        "role": "assistant", "content": null,
        "tool_calls": [
            {"id": "a1", "type": "function", "function": {"name": "list_tables", "arguments": "{\"library\":\"QGPL\"}"}},
            {"type": "function", "function": {"name": "describe_table", "arguments": {"library": "QGPL", "table": "KUNDER"}}}
        ]}}]});
        let Reply::Lookups { turn, calls } = chat.read(&reply).expect("read") else {
            panic!("lookups expected");
        };
        assert_eq!(
            calls,
            [
                (
                    "a1".to_owned(),
                    Lookup::Tables {
                        library: "QGPL".to_owned()
                    }
                ),
                (
                    "call1".to_owned(),
                    Lookup::Columns {
                        library: "QGPL".to_owned(),
                        table: "KUNDER".to_owned()
                    }
                ),
            ]
        );
        chat.answer(
            turn,
            &[
                ("a1".to_owned(), "KUNDER (table)".to_owned()),
                ("call1".to_owned(), "BY VARCHAR(30)".to_owned()),
            ],
        );
        let body = chat.body();
        let messages = body["messages"].as_array().expect("messages");
        assert_eq!(messages.len(), 5, "system, question, turn, two answers");
        assert_eq!(messages[2]["tool_calls"][1]["id"], "call1", "the id given");
        assert_eq!(messages[4]["role"], "tool");
        assert_eq!(messages[4]["tool_call_id"], "call1");

        let done = json!({"choices": [{"finish_reason": "tool_calls", "message": {"tool_calls": [
            {"id": "a2", "function": {"name": "give_sql", "arguments": "{\"sql\":\"SELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'\",\"explanation\":\"Kunderne i Århus.\"}"}}
        ]}}]});
        assert_eq!(
            chat.read(&done).expect("read"),
            Reply::Sql {
                sql: "SELECT * FROM QGPL.KUNDER WHERE BY = 'Århus'".to_owned(),
                explanation: "Kunderne i Århus.".to_owned(),
            }
        );
    }

    #[test]
    fn words_refusals_and_cut_off_answers_are_said_r_sql_48() {
        let chat = chat(Provider::Gemini);
        let words = json!({"choices": [{"finish_reason": "stop", "message": {"content": " Hvilken tabel? "}}]});
        assert_eq!(
            chat.read(&words).expect("read"),
            Reply::Text("Hvilken tabel?".to_owned())
        );
        let refused =
            json!({"choices": [{"finish_reason": "stop", "message": {"refusal": "No."}}]});
        let said = chat.read(&refused).expect_err("refused").to_string();
        assert!(said.contains("Google Gemini declined"), "{said}");
        let cut = json!({"choices": [{"finish_reason": "length", "message": {"content": ""}}]});
        assert!(chat.read(&cut).is_err());
        let broken = json!({"choices": [{"message": {"tool_calls": [
            {"id": "x", "function": {"name": "list_tables", "arguments": "{not json"}}]}}]});
        let Ok(Reply::Lookups { calls, .. }) = chat.read(&broken) else {
            panic!("a broken call is still answered");
        };
        assert_eq!(
            calls[0].1,
            Lookup::Tables {
                library: String::new()
            }
        );
    }

    #[test]
    fn a_tool_call_that_is_not_an_object_is_skipped_not_a_panic_r_sql_48() {
        let chat = chat(Provider::Mistral);
        let reply = json!({"choices": [{"finish_reason": "tool_calls", "message": {"tool_calls": [
            "list_tables",
            42,
            null,
            {"function": {"name": "list_tables", "arguments": "{\"library\":\"QGPL\"}"}}
        ]}}]});
        let Ok(Reply::Lookups { turn, calls }) = chat.read(&reply) else {
            panic!("the one good call is still read");
        };
        assert_eq!(
            calls,
            [(
                "call3".to_owned(),
                Lookup::Tables {
                    library: "QGPL".to_owned()
                }
            )]
        );
        assert_eq!(
            turn["tool_calls"].as_array().map(Vec::len),
            Some(1),
            "only the call that is answered is sent back"
        );
    }

    #[test]
    fn malformed_replies_are_no_words_not_a_panic_r_sql_48() {
        let chat = chat(Provider::OpenAi);
        for reply in [
            json!(null),
            json!([]),
            json!({"choices": "none"}),
            json!({"choices": [{"message": {"tool_calls": "list_tables"}}]}),
            json!({"choices": [{"message": {"tool_calls": [{"function": "give_sql"}]}}]}),
            json!({"choices": [{"message": {"tool_calls": [{"function": {"name": 7, "arguments": [1, 2]}}]}}]}),
        ] {
            assert!(chat.read(&reply).is_ok(), "{reply}");
        }
    }
}
