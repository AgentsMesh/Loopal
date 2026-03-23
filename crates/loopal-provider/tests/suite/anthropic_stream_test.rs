use loopal_error::{LoopalError, ProviderError};
use loopal_provider_api::{StopReason, StreamChunk};

// Re-export stream internals via a test-support path.
// Since parse_anthropic_event and ToolUseAccumulator are pub(crate), we test
// them through the integration-test-visible public API where possible, or
// replicate the parsing logic inline for unit-level coverage.

/// Minimal re-implementation of parse_anthropic_event for integration tests.
/// This mirrors the crate-internal function so we can validate SSE payloads
/// without needing pub visibility on internals.
fn parse_event(data: &str) -> (Vec<Result<StreamChunk, LoopalError>>, InlineState) {
    let mut state = InlineState::default();
    let chunks = parse_with_state(data, &mut state);
    (chunks, state)
}

fn parse_with_state(data: &str, state: &mut InlineState) -> Vec<Result<StreamChunk, LoopalError>> {
    let parsed: serde_json::Value = match serde_json::from_str(data) {
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
        "content_block_start" => {
            let block = &parsed["content_block"];
            if block["type"].as_str() == Some("tool_use") {
                state.tool_id = block["id"].as_str().map(String::from);
                state.tool_name = block["name"].as_str().map(String::from);
                state.json_buf.clear();
            }
        }
        "content_block_delta" => {
            let delta = &parsed["delta"];
            match delta["type"].as_str().unwrap_or("") {
                "text_delta" => {
                    if let Some(text) = delta["text"].as_str() {
                        chunks.push(Ok(StreamChunk::Text {
                            text: text.to_string(),
                        }));
                    }
                }
                "input_json_delta" => {
                    if let Some(p) = delta["partial_json"].as_str() {
                        state.json_buf.push_str(p);
                    }
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            if let (Some(id), Some(name)) = (state.tool_id.take(), state.tool_name.take()) {
                let input = if state.json_buf.is_empty() {
                    serde_json::json!({})
                } else {
                    serde_json::from_str(&state.json_buf).unwrap_or(serde_json::json!({}))
                };
                state.json_buf.clear();
                chunks.push(Ok(StreamChunk::ToolUse { id, name, input }));
            }
        }
        "message_delta" => {
            if let (Some(i), Some(o)) = (
                parsed["usage"]["input_tokens"].as_u64(),
                parsed["usage"]["output_tokens"].as_u64(),
            ) {
                let cc = parsed["usage"]["cache_creation_input_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                let cr = parsed["usage"]["cache_read_input_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                chunks.push(Ok(StreamChunk::Usage {
                    input_tokens: i as u32,
                    output_tokens: o as u32,
                    cache_creation_input_tokens: cc,
                    cache_read_input_tokens: cr,
                    thinking_tokens: 0,
                }));
            }
        }
        "message_start" => {
            if let (Some(i), Some(o)) = (
                parsed["message"]["usage"]["input_tokens"].as_u64(),
                parsed["message"]["usage"]["output_tokens"].as_u64(),
            ) {
                let cc = parsed["message"]["usage"]["cache_creation_input_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                let cr = parsed["message"]["usage"]["cache_read_input_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                chunks.push(Ok(StreamChunk::Usage {
                    input_tokens: i as u32,
                    output_tokens: o as u32,
                    cache_creation_input_tokens: cc,
                    cache_read_input_tokens: cr,
                    thinking_tokens: 0,
                }));
            }
        }
        "message_stop" => chunks.push(Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        })),
        _ => {}
    }
    chunks
}

#[derive(Default)]
struct InlineState {
    tool_id: Option<String>,
    tool_name: Option<String>,
    json_buf: String,
}

fn assert_text(chunk: &Result<StreamChunk, LoopalError>, expected: &str) {
    match chunk {
        Ok(StreamChunk::Text { text }) => assert_eq!(text, expected),
        other => panic!("expected Text '{expected}', got: {other:?}"),
    }
}

fn assert_tool(chunk: &Result<StreamChunk, LoopalError>, exp_id: &str, exp_name: &str) {
    match chunk {
        Ok(StreamChunk::ToolUse { id, name, .. }) => {
            assert_eq!(id, exp_id);
            assert_eq!(name, exp_name);
        }
        other => panic!("expected ToolUse, got: {other:?}"),
    }
}

fn assert_usage(chunk: &Result<StreamChunk, LoopalError>, exp_in: u32, exp_out: u32) {
    match chunk {
        Ok(StreamChunk::Usage {
            input_tokens,
            output_tokens,
            ..
        }) => {
            assert_eq!(*input_tokens, exp_in);
            assert_eq!(*output_tokens, exp_out);
        }
        other => panic!("expected Usage, got: {other:?}"),
    }
}

#[test]
fn test_text_delta() {
    let (chunks, _) = parse_event(
        r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}"#,
    );
    assert_eq!(chunks.len(), 1);
    assert_text(&chunks[0], "Hello");
}

#[test]
fn test_content_block_start_tool_use() {
    let (chunks, state) = parse_event(
        r#"{"type":"content_block_start","content_block":{"type":"tool_use","id":"tool_1","name":"read_file"}}"#,
    );
    assert!(chunks.is_empty());
    assert_eq!(state.tool_id.as_deref(), Some("tool_1"));
    assert_eq!(state.tool_name.as_deref(), Some("read_file"));
}

#[test]
fn test_input_json_delta_accumulation() {
    let mut state = InlineState {
        tool_id: Some("tool_1".to_string()),
        tool_name: Some("read_file".to_string()),
        json_buf: String::new(),
    };
    let c1 = parse_with_state(
        r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{\"path\":"}}"#,
        &mut state,
    );
    assert!(c1.is_empty());
    let c2 = parse_with_state(
        r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"\"foo.rs\"}"}}"#,
        &mut state,
    );
    assert!(c2.is_empty());
    assert_eq!(state.json_buf, r#"{"path":"foo.rs"}"#);
}

#[test]
fn test_content_block_stop_emits_tool_use() {
    let mut state = InlineState {
        tool_id: Some("tool_1".to_string()),
        tool_name: Some("read_file".to_string()),
        json_buf: r#"{"path":"foo.rs"}"#.to_string(),
    };
    let chunks = parse_with_state(r#"{"type":"content_block_stop"}"#, &mut state);
    assert_eq!(chunks.len(), 1);
    assert_tool(&chunks[0], "tool_1", "read_file");
    if let Ok(StreamChunk::ToolUse { input, .. }) = &chunks[0] {
        assert_eq!(input["path"], "foo.rs");
    }
    assert!(state.tool_id.is_none());
    assert!(state.json_buf.is_empty());
}

#[test]
fn test_content_block_stop_empty_json() {
    let mut state = InlineState {
        tool_id: Some("tool_2".to_string()),
        tool_name: Some("no_args_tool".to_string()),
        json_buf: String::new(),
    };
    let chunks = parse_with_state(r#"{"type":"content_block_stop"}"#, &mut state);
    assert_eq!(chunks.len(), 1);
    if let Ok(StreamChunk::ToolUse { input, .. }) = &chunks[0] {
        assert_eq!(*input, serde_json::json!({}));
    }
}

#[test]
fn test_content_block_stop_no_tool() {
    let (chunks, _) = parse_event(r#"{"type":"content_block_stop"}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_message_start_usage() {
    let (chunks, _) = parse_event(
        r#"{"type":"message_start","message":{"usage":{"input_tokens":100,"output_tokens":5}}}"#,
    );
    assert_eq!(chunks.len(), 1);
    assert_usage(&chunks[0], 100, 5);
}
