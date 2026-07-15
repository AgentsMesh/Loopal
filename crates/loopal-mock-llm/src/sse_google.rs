use std::time::Duration;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::sse::{event, event_raw, kind};
use crate::{MockResponse, SseAction};

pub(crate) fn plan(response: &MockResponse) -> Result<Vec<SseAction>> {
    let mut actions = Vec::new();
    for chunk in &response.chunks {
        match kind(chunk) {
            "delay" => actions.push(SseAction::Delay(Duration::from_millis(
                chunk["ms"].as_u64().unwrap_or(0),
            ))),
            "text" => actions.push(part(json!({"text": chunk["text"]}))),
            "thinking" => actions.push(part(json!({"text": chunk["text"], "thought": true}))),
            "thinking_signature" => actions.push(part(json!({
                "thoughtSignature": chunk["signature"]
            }))),
            "tool_use" | "server_tool_use" => actions.push(part(json!({
                "functionCall": {"name": chunk["name"], "args": chunk["input"]}
            }))),
            "server_tool_result" => actions.push(grounding(chunk)),
            "usage" => actions.push(usage(chunk)),
            "done" => actions.push(done(chunk)),
            "invalid_sse" => actions.push(event_raw(chunk["data"].as_str().unwrap_or("{"))),
            "disconnect" => actions.push(SseAction::Disconnect),
            other => bail!("unsupported mock chunk type: {other}"),
        }
    }
    Ok(actions)
}

fn part(value: Value) -> SseAction {
    event(json!({"candidates": [{"content": {"parts": [value]}}]}))
}

fn usage(chunk: &Value) -> SseAction {
    event(json!({
        "usageMetadata": {
            "promptTokenCount": chunk["input"].as_u64().unwrap_or(0),
            "candidatesTokenCount": chunk["output"].as_u64().unwrap_or(0),
            "thoughtsTokenCount": chunk["thinking"].as_u64().unwrap_or(0)
        }
    }))
}

fn done(chunk: &Value) -> SseAction {
    let reason = if chunk["reason"] == "max_tokens" {
        "MAX_TOKENS"
    } else {
        "STOP"
    };
    event(json!({"candidates": [{"finishReason": reason}]}))
}

fn grounding(chunk: &Value) -> SseAction {
    let chunks = chunk["content"].as_array().cloned().unwrap_or_default();
    let grounding: Vec<Value> = chunks
        .into_iter()
        .map(|item| json!({"web": {"uri": item["url"], "title": item["title"]}}))
        .collect();
    event(json!({"candidates": [{"groundingMetadata": {"groundingChunks": grounding}}]}))
}
