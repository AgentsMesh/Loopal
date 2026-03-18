use loopagent_types::error::{LoopAgentError, ProviderError};
use loopagent_types::provider::{StopReason, StreamChunk};

/// Minimal re-implementation of parse_openai_event for integration tests.
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

#[test]
fn test_tool_call_invalid_args_fallback() {
    let mut state = InlineState::default();
    state.calls.push(("call_1".to_string(), "bash".to_string(), "not json".to_string()));

    let chunks = parse_with_state(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#, &mut state);
    assert_eq!(chunks.len(), 1);
    if let Ok(StreamChunk::ToolUse { input, .. }) = &chunks[0] {
        assert_eq!(*input, serde_json::json!({}));
    }
}

#[test]
fn test_finish_reason_other_does_not_emit() {
    let (chunks, _) = parse_event(r#"{"choices":[{"delta":{},"finish_reason":"length"}]}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_multiple_tool_calls() {
    let mut state = InlineState::default();
    let _ = parse_with_state(r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"tool_a","arguments":"{}"}},{"index":1,"id":"c2","function":{"name":"tool_b","arguments":"{}"}}]},"finish_reason":null}]}"#, &mut state);
    assert_eq!(state.calls.len(), 2);

    let chunks = parse_with_state(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#, &mut state);
    assert_eq!(chunks.len(), 2);
}
