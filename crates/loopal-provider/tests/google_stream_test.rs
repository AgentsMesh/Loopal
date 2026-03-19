use loopal_types::error::{LoopalError, ProviderError};
use loopal_types::provider::{StopReason, StreamChunk};

/// Minimal re-implementation of parse_google_event for integration tests.
fn parse_event(data: &str) -> Vec<Result<StreamChunk, LoopalError>> {
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            return vec![Err(ProviderError::SseParse(format!("invalid JSON: {e}: {data}")).into())]
        }
    };

    let mut chunks = Vec::new();

    if let Some(usage) = parsed.get("usageMetadata") {
        let input = usage["promptTokenCount"].as_u64().unwrap_or(0) as u32;
        let output = usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32;
        if input > 0 || output > 0 {
            chunks.push(Ok(StreamChunk::Usage { input_tokens: input, output_tokens: output, cache_creation_input_tokens: 0, cache_read_input_tokens: 0 }));
        }
    }

    if let Some(candidates) = parsed["candidates"].as_array() {
        for candidate in candidates {
            let finish_reason = candidate["finishReason"].as_str();

            if let Some(parts) = candidate["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str()
                        && !text.is_empty() {
                            chunks.push(Ok(StreamChunk::Text { text: text.to_string() }));
                        }
                    if let Some(fc) = part.get("functionCall") {
                        let name = fc["name"].as_str().unwrap_or("").to_string();
                        let args = fc.get("args").cloned().unwrap_or(serde_json::json!({}));
                        chunks.push(Ok(StreamChunk::ToolUse {
                            id: "call_test".to_string(),
                            name,
                            input: args,
                        }));
                    }
                }
            }

            if finish_reason == Some("STOP") || finish_reason == Some("MAX_TOKENS") {
                chunks.push(Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn }));
            }
        }
    }

    chunks
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
fn test_text_part() {
    let chunks = parse_event(r#"{"candidates":[{"content":{"parts":[{"text":"Hello from Gemini"}]}}]}"#);
    assert_eq!(chunks.len(), 1);
    assert_text(&chunks[0], "Hello from Gemini");
}

#[test]
fn test_empty_text_skipped() {
    let chunks = parse_event(r#"{"candidates":[{"content":{"parts":[{"text":""}]}}]}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_function_call_emits_tool_use() {
    let chunks = parse_event(r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"read_file","args":{"path":"main.rs"}}}]}}]}"#);
    assert_eq!(chunks.len(), 1);
    match &chunks[0] {
        Ok(StreamChunk::ToolUse { name, input, .. }) => {
            assert_eq!(name, "read_file");
            assert_eq!(input["path"], "main.rs");
        }
        other => panic!("expected ToolUse, got: {:?}", other),
    }
}

#[test]
fn test_function_call_no_args() {
    let chunks = parse_event(r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"list_tools"}}]}}]}"#);
    assert_eq!(chunks.len(), 1);
    match &chunks[0] {
        Ok(StreamChunk::ToolUse { name, input, .. }) => {
            assert_eq!(name, "list_tools");
            assert_eq!(*input, serde_json::json!({}));
        }
        other => panic!("expected ToolUse, got: {:?}", other),
    }
}

#[test]
fn test_usage_metadata() {
    let chunks = parse_event(r#"{"usageMetadata":{"promptTokenCount":500,"candidatesTokenCount":120}}"#);
    assert_eq!(chunks.len(), 1);
    match &chunks[0] {
        Ok(StreamChunk::Usage { input_tokens, output_tokens, .. }) => {
            assert_eq!(*input_tokens, 500);
            assert_eq!(*output_tokens, 120);
        }
        other => panic!("expected Usage, got: {:?}", other),
    }
}

#[test]
fn test_usage_metadata_zero_tokens_skipped() {
    let chunks = parse_event(r#"{"usageMetadata":{"promptTokenCount":0,"candidatesTokenCount":0}}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_finish_reason_stop() {
    let chunks = parse_event(r#"{"candidates":[{"content":{"parts":[{"text":"Done"}]},"finishReason":"STOP"}]}"#);
    assert_eq!(chunks.len(), 2);
    assert_text(&chunks[0], "Done");
    assert_done(&chunks[1]);
}

#[test]
fn test_finish_reason_max_tokens() {
    let chunks = parse_event(r#"{"candidates":[{"content":{"parts":[{"text":"truncated"}]},"finishReason":"MAX_TOKENS"}]}"#);
    assert_eq!(chunks.len(), 2);
    assert_text(&chunks[0], "truncated");
    assert_done(&chunks[1]);
}

#[test]
fn test_invalid_json() {
    let chunks = parse_event("definitely not json");
    assert_eq!(chunks.len(), 1);
    assert!(chunks[0].is_err());
}

#[test]
fn test_no_candidates_returns_empty() {
    let chunks = parse_event(r#"{"modelVersion":"gemini-2.0-flash"}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_usage_and_candidates_combined() {
    let chunks = parse_event(r#"{"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5},"candidates":[{"content":{"parts":[{"text":"hi"}]},"finishReason":"STOP"}]}"#);
    assert_eq!(chunks.len(), 3);
    assert!(matches!(&chunks[0], Ok(StreamChunk::Usage { .. })));
    assert_text(&chunks[1], "hi");
    assert_done(&chunks[2]);
}

#[test]
fn test_usage_metadata_partial() {
    let chunks = parse_event(r#"{"usageMetadata":{"promptTokenCount":50,"candidatesTokenCount":0}}"#);
    assert_eq!(chunks.len(), 1);
    match &chunks[0] {
        Ok(StreamChunk::Usage { input_tokens, output_tokens, .. }) => {
            assert_eq!(*input_tokens, 50);
            assert_eq!(*output_tokens, 0);
        }
        other => panic!("expected Usage, got: {:?}", other),
    }
}

#[test]
fn test_candidate_without_parts() {
    let chunks = parse_event(r#"{"candidates":[{"content":{}}]}"#);
    assert!(chunks.is_empty());
}

#[test]
fn test_candidate_finish_reason_other() {
    let chunks = parse_event(r#"{"candidates":[{"content":{"parts":[{"text":"hi"}]},"finishReason":"SAFETY"}]}"#);
    assert_eq!(chunks.len(), 1);
    assert_text(&chunks[0], "hi");
}
