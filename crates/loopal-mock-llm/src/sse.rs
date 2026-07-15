use std::time::Duration;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::MockResponse;

#[derive(Clone, Debug, PartialEq)]
pub enum SseAction {
    Delay(Duration),
    Event(String),
    Disconnect,
}

pub fn plan_sse(response: &MockResponse) -> Result<Vec<SseAction>> {
    if !response.raw_sse.is_empty() {
        return Ok(response
            .raw_sse
            .iter()
            .map(|data| event_raw(data))
            .collect());
    }
    let mut actions = vec![event(crate::sse_usage::message_start(&response.chunks))];
    let mut block = 0usize;
    let mut index = 0usize;
    while index < response.chunks.len() {
        let chunk = &response.chunks[index];
        match kind(chunk) {
            "delay" => actions.push(SseAction::Delay(Duration::from_millis(
                chunk["ms"].as_u64().unwrap_or(0),
            ))),
            "text" => {
                actions.push(event(json!({
                    "type": "content_block_start", "index": block,
                    "content_block": {"type": "text", "text": ""}
                })));
                while index < response.chunks.len() {
                    let item = &response.chunks[index];
                    match kind(item) {
                        "text" => actions.push(event(json!({
                            "type": "content_block_delta", "index": block,
                            "delta": {"type": "text_delta", "text": item["text"].as_str().unwrap_or("")}
                        }))),
                        "delay" => actions.push(SseAction::Delay(Duration::from_millis(
                            item["ms"].as_u64().unwrap_or(0),
                        ))),
                        _ => break,
                    }
                    index += 1;
                }
                actions.push(block_stop(block));
                block += 1;
                continue;
            }
            "thinking" | "thinking_signature" => {
                actions.push(event(json!({
                    "type": "content_block_start", "index": block,
                    "content_block": {"type": "thinking", "thinking": ""}
                })));
                while index < response.chunks.len() {
                    let item = &response.chunks[index];
                    match kind(item) {
                        "thinking" => actions.push(event(json!({
                            "type": "content_block_delta", "index": block,
                            "delta": {"type": "thinking_delta", "thinking": item["text"].as_str().unwrap_or("")}
                        }))),
                        "thinking_signature" => actions.push(event(json!({
                            "type": "content_block_delta", "index": block,
                            "delta": {"type": "signature_delta", "signature": item["signature"].as_str().unwrap_or("mock-signature")}
                        }))),
                        "delay" => actions.push(SseAction::Delay(Duration::from_millis(
                            item["ms"].as_u64().unwrap_or(0),
                        ))),
                        _ => break,
                    }
                    index += 1;
                }
                actions.push(block_stop(block));
                block += 1;
                continue;
            }
            "tool_use" => {
                actions.extend(tool_block(block, chunk));
                block += 1;
            }
            "server_tool_use" => {
                actions.extend(server_tool_block(block, chunk));
                block += 1;
            }
            "server_tool_result" => {
                actions.push(event(json!({
                    "type": "content_block_start", "index": block,
                    "content_block": {
                        "type": chunk["block_type"].as_str().unwrap_or("web_search_tool_result"),
                        "tool_use_id": chunk["tool_use_id"].as_str().unwrap_or("server-tool"),
                        "content": chunk.get("content").cloned().unwrap_or(Value::Null)
                    }
                })));
                actions.push(block_stop(block));
                block += 1;
            }
            "usage"
                if response
                    .chunks
                    .get(index + 1)
                    .is_none_or(|next| kind(next) != "done") =>
            {
                actions.push(event(crate::sse_usage::message_delta(Some(chunk), None)));
            }
            "usage" => {}
            "done" => {
                let usage = index
                    .checked_sub(1)
                    .and_then(|previous| response.chunks.get(previous))
                    .filter(|previous| kind(previous) == "usage");
                actions.push(event(crate::sse_usage::message_delta(
                    usage,
                    Some(chunk["reason"].as_str().unwrap_or("end_turn")),
                )));
                actions.push(event(json!({"type": "message_stop"})));
            }
            "invalid_sse" => actions.push(event_raw(chunk["data"].as_str().unwrap_or("{"))),
            "disconnect" => actions.push(SseAction::Disconnect),
            other => bail!("unsupported mock chunk type: {other}"),
        }
        index += 1;
    }
    Ok(actions)
}

fn tool_block(index: usize, chunk: &Value) -> Vec<SseAction> {
    let mut actions = vec![event(json!({
        "type": "content_block_start", "index": index,
        "content_block": {"type": "tool_use", "id": chunk["id"].as_str().unwrap_or("tool-1"), "name": chunk["name"].as_str().unwrap_or("")}
    }))];
    actions.extend(input_deltas(index, chunk));
    actions.push(block_stop(index));
    actions
}

fn server_tool_block(index: usize, chunk: &Value) -> Vec<SseAction> {
    let mut actions = vec![event(json!({
        "type": "content_block_start", "index": index,
        "content_block": {"type": "server_tool_use", "id": chunk["id"].as_str().unwrap_or("server-tool"), "name": chunk["name"].as_str().unwrap_or("web_search"), "input": {}}
    }))];
    actions.extend(input_deltas(index, chunk));
    actions.push(block_stop(index));
    actions
}

fn input_deltas(index: usize, chunk: &Value) -> Vec<SseAction> {
    let fallback = chunk
        .get("input")
        .cloned()
        .unwrap_or_else(|| json!({}))
        .to_string();
    let fragments = chunk["inputFragments"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| vec![fallback]);
    fragments
        .into_iter()
        .map(|partial| {
            event(json!({
                "type": "content_block_delta", "index": index,
                "delta": {"type": "input_json_delta", "partial_json": partial}
            }))
        })
        .collect()
}

fn block_stop(index: usize) -> SseAction {
    event(json!({"type": "content_block_stop", "index": index}))
}

pub(crate) fn event(value: Value) -> SseAction {
    event_raw(&value.to_string())
}
pub(crate) fn event_raw(data: &str) -> SseAction {
    SseAction::Event(format!("data: {data}\n\n"))
}
pub(crate) fn kind(value: &Value) -> &str {
    value["type"].as_str().unwrap_or("")
}
