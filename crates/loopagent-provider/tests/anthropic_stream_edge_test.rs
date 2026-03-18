use loopagent_types::error::{LoopAgentError, ProviderError};
use loopagent_types::provider::{StopReason, StreamChunk};

fn parse_event(data: &str) -> (Vec<Result<StreamChunk, LoopAgentError>>, InlineState) {
    let mut state = InlineState::default();
    let chunks = parse_with_state(data, &mut state);
    (chunks, state)
}

fn parse_with_state(
    data: &str,
    state: &mut InlineState,
) -> Vec<Result<StreamChunk, LoopAgentError>> {
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            return vec![Err(
                ProviderError::SseParse(format!("invalid JSON: {e}: {data}")).into(),
            )]
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
                        chunks.push(Ok(StreamChunk::Text { text: text.to_string() }));
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
                let cc = parsed["usage"]["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32;
                let cr = parsed["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32;
                chunks.push(Ok(StreamChunk::Usage {
                    input_tokens: i as u32, output_tokens: o as u32,
                    cache_creation_input_tokens: cc, cache_read_input_tokens: cr,
                }));
            }
        }
        "message_start" => {
            if let (Some(i), Some(o)) = (
                parsed["message"]["usage"]["input_tokens"].as_u64(),
                parsed["message"]["usage"]["output_tokens"].as_u64(),
            ) {
                let cc = parsed["message"]["usage"]["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32;
                let cr = parsed["message"]["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32;
                chunks.push(Ok(StreamChunk::Usage {
                    input_tokens: i as u32, output_tokens: o as u32,
                    cache_creation_input_tokens: cc, cache_read_input_tokens: cr,
                }));
            }
        }
        "message_stop" => chunks.push(Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn })),
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

#[test]
fn test_message_delta_usage() {
    let (chunks, _) = parse_event(r#"{"type":"message_delta","usage":{"input_tokens":200,"output_tokens":50}}"#);
    assert_eq!(chunks.len(), 1);
    match &chunks[0] {
        Ok(StreamChunk::Usage { input_tokens, output_tokens, .. }) => {
            assert_eq!(*input_tokens, 200);
            assert_eq!(*output_tokens, 50);
        }
        other => panic!("expected Usage, got: {:?}", other),
    }
}

#[test]
fn test_message_stop() {
    let (chunks, _) = parse_event(r#"{"type":"message_stop"}"#);
    assert_eq!(chunks.len(), 1);
    assert!(matches!(&chunks[0], Ok(StreamChunk::Done { .. })), "expected Done, got: {:?}", &chunks[0]);
}

#[test]
fn test_invalid_json() {
    let (chunks, _) = parse_event("not valid json");
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].is_err());
}

#[test]
fn test_unknown_event_type() {
    let (chunks, _) = parse_event(r#"{"type":"ping"}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_content_block_start_text() {
    let (chunks, state) = parse_event(r#"{"type":"content_block_start","content_block":{"type":"text","text":""}}"#);
    assert!(chunks.is_empty());
    assert!(state.tool_id.is_none());
}

#[test]
fn test_content_block_stop_invalid_json_fragments() {
    let mut state = InlineState {
        tool_id: Some("tool_x".to_string()),
        tool_name: Some("test_tool".to_string()),
        json_buf: "not valid json{".to_string(),
    };
    let chunks = parse_with_state(r#"{"type":"content_block_stop"}"#, &mut state);
    assert_eq!(chunks.len(), 1);
    if let Ok(StreamChunk::ToolUse { input, .. }) = &chunks[0] {
        assert_eq!(*input, serde_json::json!({}));
    }
}

#[test]
fn test_message_delta_without_usage() {
    let (chunks, _) = parse_event(r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_message_start_without_usage() {
    let (chunks, _) = parse_event(r#"{"type":"message_start","message":{"id":"msg_1"}}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_text_delta_missing_text_field() {
    let (chunks, _) = parse_event(r#"{"type":"content_block_delta","delta":{"type":"text_delta"}}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_input_json_delta_missing_partial_json() {
    let mut state = InlineState {
        tool_id: Some("t1".to_string()),
        tool_name: Some("tool".to_string()),
        json_buf: String::new(),
    };
    let chunks = parse_with_state(r#"{"type":"content_block_delta","delta":{"type":"input_json_delta"}}"#, &mut state);
    assert!(chunks.is_empty());
    assert!(state.json_buf.is_empty());
}

#[test]
fn test_unknown_delta_type() {
    let (chunks, _) = parse_event(r#"{"type":"content_block_delta","delta":{"type":"unknown_delta"}}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_full_tool_use_flow() {
    let mut state = InlineState::default();
    let c = parse_with_state(r#"{"type":"content_block_start","content_block":{"type":"tool_use","id":"call_abc","name":"bash"}}"#, &mut state);
    assert!(c.is_empty());
    let c = parse_with_state(r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{\"cmd\":\"ls"}}"#, &mut state);
    assert!(c.is_empty());
    let c = parse_with_state(r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":" -la\"}"}}"#, &mut state);
    assert!(c.is_empty());
    let c = parse_with_state(r#"{"type":"content_block_stop"}"#, &mut state);
    assert_eq!(c.len(), 1);
    match &c[0] {
        Ok(StreamChunk::ToolUse { id, name, input }) => {
            assert_eq!(id, "call_abc");
            assert_eq!(name, "bash");
            assert_eq!(input["cmd"], "ls -la");
        }
        other => panic!("expected ToolUse, got: {:?}", other),
    }
}
