use std::time::Duration;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::sse::{event, event_raw, kind};
use crate::{MockResponse, SseAction};

pub(crate) fn plan(response: &MockResponse) -> Result<Vec<SseAction>> {
    let mut actions = Vec::new();
    let mut usage = None;
    for chunk in &response.chunks {
        match kind(chunk) {
            "delay" => actions.push(SseAction::Delay(Duration::from_millis(
                chunk["ms"].as_u64().unwrap_or(0),
            ))),
            "text" => actions.push(event(json!({
                "type": "response.output_text.delta",
                "delta": chunk["text"].as_str().unwrap_or("")
            }))),
            "thinking" => actions.push(event(json!({
                "type": "response.reasoning_summary_text.delta",
                "delta": chunk["text"].as_str().unwrap_or("")
            }))),
            "thinking_signature" => actions.push(event(json!({
                "type": "response.output_item.done",
                "item": {"type": "reasoning", "id": chunk["signature"]}
            }))),
            "tool_use" => actions.push(function_call(chunk)),
            "server_tool_use" => actions.push(web_search(chunk)),
            "server_tool_result" => {}
            "usage" => usage = Some(chunk.clone()),
            "done" => actions.push(done(chunk, usage.as_ref())),
            "invalid_sse" => actions.push(event_raw(chunk["data"].as_str().unwrap_or("{"))),
            "disconnect" => actions.push(SseAction::Disconnect),
            other => bail!("unsupported mock chunk type: {other}"),
        }
    }
    Ok(actions)
}

fn function_call(chunk: &Value) -> SseAction {
    event(json!({
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "call_id": chunk["id"],
            "name": chunk["name"],
            "arguments": chunk["input"].to_string()
        }
    }))
}

fn web_search(chunk: &Value) -> SseAction {
    event(json!({
        "type": "response.output_item.done",
        "item": {
            "type": "web_search_call", "id": chunk["id"],
            "action": {"type": "search", "query": chunk["input"]["query"]}
        }
    }))
}

fn done(chunk: &Value, usage: Option<&Value>) -> SseAction {
    if chunk["reason"]
        .as_str()
        .is_some_and(|reason| reason != "end_turn")
    {
        return event(json!({
            "type": "response.incomplete",
            "response": {
                "status": "incomplete",
                "incomplete_details": {"reason": "max_output_tokens"},
                "usage": usage_json(usage)
            }
        }));
    }
    event(json!({
        "type": "response.completed",
        "response": {"usage": usage_json(usage)}
    }))
}

fn usage_json(usage: Option<&Value>) -> Value {
    json!({
        "input_tokens": field(usage, "input"),
        "output_tokens": field(usage, "output"),
        "input_tokens_details": {"cached_tokens": field(usage, "cache_read")},
        "output_tokens_details": {"reasoning_tokens": field(usage, "thinking")}
    })
}

fn field(usage: Option<&Value>, name: &str) -> u64 {
    usage.and_then(|value| value[name].as_u64()).unwrap_or(0)
}
