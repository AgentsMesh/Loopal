use std::time::Duration;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::sse::{event, event_raw, kind};
use crate::{MockResponse, SseAction};

pub(crate) fn plan(response: &MockResponse) -> Result<Vec<SseAction>> {
    let mut actions = Vec::new();
    let mut tool_index = 0usize;
    let mut saw_tool = false;
    let mut final_usage = None;
    for chunk in &response.chunks {
        match kind(chunk) {
            "delay" => actions.push(SseAction::Delay(Duration::from_millis(
                chunk["ms"].as_u64().unwrap_or(0),
            ))),
            "text" => actions.push(delta(json!({"content": chunk["text"]}))),
            "thinking" => actions.push(delta(json!({"reasoning_content": chunk["text"]}))),
            "thinking_signature" => {}
            "tool_use" | "server_tool_use" => {
                actions.extend(tool_deltas(tool_index, chunk));
                tool_index += 1;
                saw_tool = true;
            }
            "server_tool_result" => {}
            "usage" => final_usage = Some(chunk.clone()),
            "done" => {
                actions.push(finish(chunk, saw_tool));
                if let Some(value) = final_usage.as_ref() {
                    actions.push(usage(value));
                }
                actions.push(event_raw("[DONE]"));
            }
            "invalid_sse" => actions.push(event_raw(chunk["data"].as_str().unwrap_or("{"))),
            "disconnect" => actions.push(SseAction::Disconnect),
            other => bail!("unsupported mock chunk type: {other}"),
        }
    }
    Ok(actions)
}

fn delta(value: Value) -> SseAction {
    event(json!({"choices": [{"delta": value, "finish_reason": null}]}))
}

fn tool_deltas(index: usize, chunk: &Value) -> Vec<SseAction> {
    let fragments = fragments(chunk);
    fragments
        .into_iter()
        .enumerate()
        .map(|(position, arguments)| {
            let call = if position == 0 {
                json!({
                    "index": index, "id": chunk["id"], "type": "function",
                    "function": {"name": chunk["name"], "arguments": arguments}
                })
            } else {
                json!({"index": index, "function": {"arguments": arguments}})
            };
            delta(json!({"tool_calls": [call]}))
        })
        .collect()
}

fn fragments(chunk: &Value) -> Vec<String> {
    chunk["inputFragments"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| vec![chunk["input"].to_string()])
}

fn usage(chunk: &Value) -> SseAction {
    event(json!({
        "choices": [],
        "usage": {
            "prompt_tokens": chunk["input"].as_u64().unwrap_or(0),
            "completion_tokens": chunk["output"].as_u64().unwrap_or(0),
            "completion_tokens_details": {
                "reasoning_tokens": chunk["thinking"].as_u64().unwrap_or(0)
            }
        }
    }))
}

fn finish(chunk: &Value, saw_tool: bool) -> SseAction {
    let reason = match chunk["reason"].as_str().unwrap_or("end_turn") {
        "max_tokens" => "length",
        _ if saw_tool => "tool_calls",
        _ => "stop",
    };
    event(json!({"choices": [{"delta": {}, "finish_reason": reason}]}))
}
