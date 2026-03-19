use loopal_types::error::{LoopalError, ProviderError};
use loopal_types::provider::{StopReason, StreamChunk};

/// Minimal re-implementation of parse_openai_event for integration tests.
fn parse_event(data: &str) -> (Vec<Result<StreamChunk, LoopalError>>, InlineState) {
    let mut state = InlineState::default();
    let chunks = parse_with_state(data, &mut state);
    (chunks, state)
}

fn parse_with_state(
    data: &str,
    state: &mut InlineState,
) -> Vec<Result<StreamChunk, LoopalError>> {
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            return vec![Err(ProviderError::SseParse(format!("invalid JSON: {e}: {data}")).into())]
        }
    };

    let mut chunks = Vec::new();

    if let Some(usage) = parsed.get("usage").filter(|u| !u.is_null())
        && let (Some(i), Some(o)) = (usage["prompt_tokens"].as_u64(), usage["completion_tokens"].as_u64()) {
            chunks.push(Ok(StreamChunk::Usage { input_tokens: i as u32, output_tokens: o as u32, cache_creation_input_tokens: 0, cache_read_input_tokens: 0 }));
        }

    let choices = match parsed["choices"].as_array() {
        Some(c) => c,
        None => return chunks,
    };

    for choice in choices {
        let delta = &choice["delta"];
        let finish_reason = choice["finish_reason"].as_str();

        if let Some(content) = delta["content"].as_str()
            && !content.is_empty() {
                chunks.push(Ok(StreamChunk::Text { text: content.to_string() }));
            }

        if let Some(tool_calls) = delta["tool_calls"].as_array() {
            for tc in tool_calls {
                let index = tc["index"].as_u64().unwrap_or(0) as usize;
                if index > 128 { continue; }
                while state.calls.len() <= index {
                    state.calls.push((String::new(), String::new(), String::new()));
                }
                if let Some(id) = tc["id"].as_str() { state.calls[index].0 = id.to_string(); }
                if let Some(name) = tc["function"]["name"].as_str() { state.calls[index].1 = name.to_string(); }
                if let Some(args) = tc["function"]["arguments"].as_str() { state.calls[index].2.push_str(args); }
            }
        }

        if finish_reason == Some("tool_calls") || finish_reason == Some("stop") {
            for (id, name, args) in state.calls.drain(..) {
                if !id.is_empty() && !name.is_empty() {
                    let input = serde_json::from_str(&args).unwrap_or(serde_json::json!({}));
                    chunks.push(Ok(StreamChunk::ToolUse { id, name, input }));
                }
            }
            if finish_reason == Some("stop") {
                chunks.push(Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn }));
            }
        }
    }

    chunks
}

#[derive(Default)]
struct InlineState {
    calls: Vec<(String, String, String)>,
}

fn assert_text(chunk: &Result<StreamChunk, LoopalError>, expected: &str) {
    match chunk {
        Ok(StreamChunk::Text { text }) => assert_eq!(text, expected),
        other => panic!("expected Text '{}', got: {:?}", expected, other),
    }
}

fn assert_done(chunk: &Result<StreamChunk, LoopalError>) {
    assert!(matches!(chunk, Ok(StreamChunk::Done { .. })), "expected Done, got: {:?}", chunk);
}

#[test]
fn test_text_content_delta() {
    let (chunks, _) = parse_event(r#"{"choices":[{"delta":{"content":"Hello world"},"finish_reason":null}]}"#);
    assert_eq!(chunks.len(), 1);
    assert_text(&chunks[0], "Hello world");
}

#[test]
fn test_empty_content_skipped() {
    let (chunks, _) = parse_event(r#"{"choices":[{"delta":{"content":""},"finish_reason":null}]}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_tool_calls_accumulation() {
    let mut state = InlineState::default();
    let c1 = parse_with_state(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":""}}]},"finish_reason":null}]}"#, &mut state);
    assert!(c1.is_empty());
    assert_eq!(state.calls[0].0, "call_1");
    assert_eq!(state.calls[0].1, "read_file");

    let _ = parse_with_state(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]},"finish_reason":null}]}"#, &mut state);
    let _ = parse_with_state(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"foo.rs\"}"}}]},"finish_reason":null}]}"#, &mut state);
    assert_eq!(state.calls[0].2, r#"{"path":"foo.rs"}"#);
}

#[test]
fn test_finish_reason_tool_calls_emits_accumulated() {
    let mut state = InlineState::default();
    state.calls.push(("call_1".to_string(), "bash".to_string(), r#"{"cmd":"ls"}"#.to_string()));

    let chunks = parse_with_state(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#, &mut state);
    assert_eq!(chunks.len(), 1);
    match &chunks[0] {
        Ok(StreamChunk::ToolUse { id, name, input }) => {
            assert_eq!(id, "call_1");
            assert_eq!(name, "bash");
            assert_eq!(input["cmd"], "ls");
        }
        other => panic!("expected ToolUse, got: {:?}", other),
    }
    assert!(state.calls.is_empty());
}

#[test]
fn test_finish_reason_stop_emits_done() {
    let (chunks, _) = parse_event(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#);
    assert_eq!(chunks.len(), 1);
    assert_done(&chunks[0]);
}

#[test]
fn test_finish_reason_stop_with_accumulated_tools() {
    let mut state = InlineState::default();
    state.calls.push(("call_x".to_string(), "tool_x".to_string(), "{}".to_string()));

    let chunks = parse_with_state(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#, &mut state);
    assert_eq!(chunks.len(), 2);
    match &chunks[0] {
        Ok(StreamChunk::ToolUse { id, name, .. }) => { assert_eq!(id, "call_x"); assert_eq!(name, "tool_x"); }
        other => panic!("expected ToolUse, got: {:?}", other),
    }
    assert_done(&chunks[1]);
}

#[test]
fn test_usage_stats() {
    let (chunks, _) = parse_event(r#"{"usage":{"prompt_tokens":150,"completion_tokens":42},"choices":[]}"#);
    assert_eq!(chunks.len(), 1);
    match &chunks[0] {
        Ok(StreamChunk::Usage { input_tokens, output_tokens, .. }) => {
            assert_eq!(*input_tokens, 150);
            assert_eq!(*output_tokens, 42);
        }
        other => panic!("expected Usage, got: {:?}", other),
    }
}

#[test]
fn test_invalid_json() {
    let (chunks, _) = parse_event("not json at all");
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].is_err());
}

#[test]
fn test_large_tool_call_index_skipped() {
    let (chunks, state) = parse_event(r#"{"choices":[{"delta":{"tool_calls":[{"index":200,"id":"call_big","function":{"name":"evil","arguments":"{}"}}]},"finish_reason":null}]}"#);
    assert!(chunks.is_empty());
    assert!(state.calls.is_empty());
}

#[test]
fn test_no_choices_returns_empty() {
    let (chunks, _) = parse_event(r#"{"id":"chatcmpl-1"}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_usage_null_is_ignored() {
    let (chunks, _) = parse_event(r#"{"usage":null,"choices":[{"delta":{"content":"text"},"finish_reason":null}]}"#);
    assert_eq!(chunks.len(), 1);
    assert_text(&chunks[0], "text");
}

#[test]
fn test_finish_tool_calls_skips_empty_id_name() {
    let mut state = InlineState::default();
    state.calls.push(("".to_string(), "".to_string(), "{}".to_string()));

    let chunks = parse_with_state(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#, &mut state);
    assert!(chunks.is_empty(), "empty id+name tool calls should be skipped");
}
