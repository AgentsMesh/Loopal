//! Unit tests for stream truncation detection in `stream_llm_with`.
//!
//! These test the llm.rs `received_done` / `stream_error` logic directly,
//! without going through the full turn execution loop.

use loopal_provider_api::{StopReason, StreamChunk};

use super::{make_cancel, make_runner_with_mock_provider};

/// EOF after text chunks but no Done → stream_error=true (silent truncation).
#[tokio::test]
async fn test_eof_with_text_no_done_sets_stream_error() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "Let me create the file.".into(),
        }),
        // Stream ends — no Done chunk
    ];
    let (mut runner, _rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);
    let msgs = runner.params.store.messages().to_vec();
    let cancel = make_cancel();

    let result = runner.stream_llm_with(&msgs, None, &cancel).await.unwrap();

    assert_eq!(result.assistant_text, "Let me create the file.");
    assert!(
        result.stream_error,
        "EOF without Done should set stream_error"
    );
    assert!(result.tool_uses.is_empty());
    // stop_reason stays at default EndTurn (Done never arrived to set it)
    assert_eq!(result.stop_reason, StopReason::EndTurn);
}

/// EOF after text + complete tool_use but no Done → stream_error=true,
/// tool_use is still collected (tool JSON was complete at content_block_stop).
#[tokio::test]
async fn test_eof_with_text_and_tool_no_done_sets_stream_error() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "Let me read.".into(),
        }),
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/test.txt"}),
        }),
        // No Done — stream truncated after tool_use
    ];
    let (mut runner, _rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);
    let msgs = runner.params.store.messages().to_vec();
    let cancel = make_cancel();

    let result = runner.stream_llm_with(&msgs, None, &cancel).await.unwrap();

    assert_eq!(result.assistant_text, "Let me read.");
    assert!(result.stream_error);
    // Tool was fully parsed (content_block_stop emitted ToolUse chunk)
    assert_eq!(result.tool_uses.len(), 1);
    assert_eq!(result.tool_uses[0].1, "Read");
}

/// Normal response with Done → stream_error=false.
/// Regression guard: ensure normal flow is unaffected.
#[tokio::test]
async fn test_normal_response_with_done_no_stream_error() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "All done.".into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ];
    let (mut runner, _rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);
    let msgs = runner.params.store.messages().to_vec();
    let cancel = make_cancel();

    let result = runner.stream_llm_with(&msgs, None, &cancel).await.unwrap();

    assert_eq!(result.assistant_text, "All done.");
    assert!(!result.stream_error);
    assert_eq!(result.stop_reason, StopReason::EndTurn);
}

/// Explicit Err chunk (e.g. StreamEnded) + partial text → stream_error=true.
/// The Err handler (not the truncation detector) sets stream_error.
#[tokio::test]
async fn test_err_chunk_with_text_sets_stream_error() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "partial".into(),
        }),
        Err(loopal_error::LoopalError::Provider(
            loopal_error::ProviderError::StreamEnded,
        )),
    ];
    let (mut runner, _rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);
    let msgs = runner.params.store.messages().to_vec();
    let cancel = make_cancel();

    let result = runner.stream_llm_with(&msgs, None, &cancel).await.unwrap();

    assert_eq!(result.assistant_text, "partial");
    assert!(result.stream_error);
    // Truncation detector skips because stream_error already set by Err handler
}

/// MaxTokens Done → stream_error=false, stop_reason=MaxTokens.
/// Regression guard: MaxTokens should not be confused with truncation.
#[tokio::test]
async fn test_max_tokens_done_not_confused_with_truncation() {
    let chunks = vec![
        Ok(StreamChunk::Text {
            text: "long text...".into(),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::MaxTokens,
        }),
    ];
    let (mut runner, _rx, _mbox, _ctrl) = make_runner_with_mock_provider(chunks);
    let msgs = runner.params.store.messages().to_vec();
    let cancel = make_cancel();

    let result = runner.stream_llm_with(&msgs, None, &cancel).await.unwrap();

    assert!(
        !result.stream_error,
        "MaxTokens with Done is not truncation"
    );
    assert_eq!(result.stop_reason, StopReason::MaxTokens);
}
