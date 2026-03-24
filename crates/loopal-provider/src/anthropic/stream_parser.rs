use super::accumulator::{ServerToolAccumulator, ThinkingAccumulator, ToolUseAccumulator};
use super::server_tool;
use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{StopReason, StreamChunk};
use serde_json::{Value, json};

/// Parse a single SSE event into zero or more `StreamChunk`s.
pub(crate) fn parse_anthropic_event(
    data: &str,
    tool: &mut ToolUseAccumulator,
    thinking: &mut ThinkingAccumulator,
    server: &mut ServerToolAccumulator,
) -> Vec<Result<StreamChunk, LoopalError>> {
    let parsed: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            return vec![Err(ProviderError::SseParse(format!(
                "invalid JSON: {e}: {data}"
            ))
            .into())];
        }
    };

    let event_type = parsed["type"].as_str().unwrap_or("");
    let mut chunks = Vec::new();

    match event_type {
        "content_block_start" => handle_block_start(&parsed, tool, thinking, server),
        "content_block_delta" => handle_block_delta(&parsed, tool, thinking, &mut chunks),
        "content_block_stop" => handle_block_stop(tool, thinking, server, &mut chunks),
        "message_delta" => parse_usage_and_stop(&parsed, tool, &mut chunks),
        "message_start" => parse_message_start_usage(&parsed, &mut chunks),
        "message_stop" => {
            let reason = tool.stop_reason.take().unwrap_or(StopReason::EndTurn);
            chunks.push(Ok(StreamChunk::Done {
                stop_reason: reason,
            }));
        }
        _ => {}
    }

    chunks
}

fn handle_block_start(
    parsed: &Value,
    tool: &mut ToolUseAccumulator,
    thinking: &mut ThinkingAccumulator,
    server: &mut ServerToolAccumulator,
) {
    let block = &parsed["content_block"];
    let block_type = block["type"].as_str().unwrap_or("");
    match block_type {
        "tool_use" => {
            tool.current_tool_id = block["id"].as_str().map(String::from);
            tool.current_tool_name = block["name"].as_str().map(String::from);
            tool.json_fragments.clear();
        }
        "thinking" => {
            thinking.active = true;
            thinking.signature_fragments.clear();
        }
        server_tool::SERVER_TOOL_USE_TYPE => {
            let input = block.get("input").cloned().unwrap_or(json!({}));
            server.current = Some((
                block["id"].as_str().unwrap_or("").to_string(),
                block["name"].as_str().unwrap_or("").to_string(),
                input,
            ));
            server.is_result = false;
        }
        // Any server-side tool result: web_search_tool_result, code_execution_tool_result, etc.
        other if other.ends_with("_tool_result") && other != "tool_result" => {
            server.result_block_type = Some(other.to_string());
            server.result_tool_use_id = block["tool_use_id"].as_str().map(String::from);
            server.result_content = Some(block["content"].clone());
            server.is_result = true;
        }
        _ => {}
    }
}

fn handle_block_delta(
    parsed: &Value,
    tool: &mut ToolUseAccumulator,
    thinking: &mut ThinkingAccumulator,
    chunks: &mut Vec<Result<StreamChunk, LoopalError>>,
) {
    let delta = &parsed["delta"];
    let delta_type = delta["type"].as_str().unwrap_or("");
    match delta_type {
        "text_delta" => {
            if let Some(text) = delta["text"].as_str() {
                chunks.push(Ok(StreamChunk::Text {
                    text: text.to_string(),
                }));
            }
        }
        "input_json_delta" => {
            if let Some(partial) = delta["partial_json"].as_str() {
                tool.json_fragments.push_str(partial);
            }
        }
        "thinking_delta" => {
            if let Some(text) = delta["thinking"].as_str() {
                chunks.push(Ok(StreamChunk::Thinking {
                    text: text.to_string(),
                }));
            }
        }
        "signature_delta" => {
            if let Some(sig) = delta["signature"].as_str() {
                thinking.signature_fragments.push_str(sig);
            }
        }
        _ => {}
    }
}

fn handle_block_stop(
    tool: &mut ToolUseAccumulator,
    thinking: &mut ThinkingAccumulator,
    server: &mut ServerToolAccumulator,
    chunks: &mut Vec<Result<StreamChunk, LoopalError>>,
) {
    if thinking.active {
        let sig = if thinking.signature_fragments.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut thinking.signature_fragments))
        };
        if let Some(signature) = sig {
            chunks.push(Ok(StreamChunk::ThinkingSignature { signature }));
        }
        thinking.active = false;
    } else if server.is_result {
        // Emit server tool result
        if let Some(tool_use_id) = server.result_tool_use_id.take() {
            let content = server.result_content.take().unwrap_or(json!(null));
            let block_type = server
                .result_block_type
                .take()
                .unwrap_or_else(|| "unknown_tool_result".to_string());
            chunks.push(Ok(StreamChunk::ServerToolResult {
                block_type,
                tool_use_id,
                content,
            }));
        }
        server.is_result = false;
    } else if let Some((id, name, input)) = server.current.take() {
        // Emit server tool use (input captured at content_block_start)
        chunks.push(Ok(StreamChunk::ServerToolUse { id, name, input }));
    } else if let (Some(id), Some(name)) =
        (tool.current_tool_id.take(), tool.current_tool_name.take())
    {
        let input: Value = if tool.json_fragments.is_empty() {
            json!({})
        } else {
            serde_json::from_str(&tool.json_fragments).unwrap_or(json!({}))
        };
        tool.json_fragments.clear();
        chunks.push(Ok(StreamChunk::ToolUse { id, name, input }));
    }
}

fn parse_usage_and_stop(
    parsed: &Value,
    tool: &mut ToolUseAccumulator,
    chunks: &mut Vec<Result<StreamChunk, LoopalError>>,
) {
    push_usage_from(&parsed["usage"], chunks);
    if let Some(reason) = parsed["delta"]["stop_reason"].as_str() {
        tool.stop_reason = match reason {
            "max_tokens" => Some(StopReason::MaxTokens),
            "pause_turn" => Some(StopReason::PauseTurn),
            _ => Some(StopReason::EndTurn),
        };
    }
}

fn parse_message_start_usage(parsed: &Value, chunks: &mut Vec<Result<StreamChunk, LoopalError>>) {
    push_usage_from(&parsed["message"]["usage"], chunks);
}
fn push_usage_from(usage: &Value, chunks: &mut Vec<Result<StreamChunk, LoopalError>>) {
    if let (Some(input), Some(output)) = (
        usage["input_tokens"].as_u64(),
        usage["output_tokens"].as_u64(),
    ) {
        let cache_creation = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32;
        let cache_read = usage["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32;
        chunks.push(Ok(StreamChunk::Usage {
            input_tokens: input as u32,
            output_tokens: output as u32,
            cache_creation_input_tokens: cache_creation,
            cache_read_input_tokens: cache_read,
            thinking_tokens: 0,
        }));
    }
}
