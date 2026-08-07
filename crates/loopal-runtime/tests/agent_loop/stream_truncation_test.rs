//! Integration tests for stream truncation auto-continue.
//!
//! Full turn_exec loop: truncation → record partial → auto-continue.

use loopal_error::TerminateReason;
use loopal_provider_api::{StopReason, StreamChunk};

use super::mock_provider::make_multi_runner;

/// Explicit Err (e.g. StreamEnded) after partial text → auto-continue
/// → second LLM call completes normally.
#[tokio::test]
async fn test_err_with_text_triggers_auto_continue() {
    let calls = vec![
        // First LLM call: text then error (no Done)
        vec![
            Ok(StreamChunk::Text {
                text: "Let me check.".into(),
            }),
            Err(loopal_error::LoopalError::Provider(
                loopal_error::ProviderError::StreamEnded,
            )),
        ],
        // Second LLM call (auto-continue): model finishes
        vec![
            Ok(StreamChunk::Text {
                text: "Here is the result.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "Here is the result.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}

/// Err after text + complete tool_use → tool is discarded (may be subset
/// of intended tools), auto-continue → model re-issues the tool call.
#[tokio::test]
async fn test_err_with_tool_discards_and_continues() {
    let tmp = std::env::temp_dir().join(format!("la_errtool_{}.txt", std::process::id()));
    std::fs::write(&tmp, "data").unwrap();
    let calls = vec![
        // First LLM: text + tool + Err (proxy dropped after first tool)
        vec![
            Ok(StreamChunk::Text {
                text: "Reading file.".into(),
            }),
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
            }),
            Err(loopal_error::LoopalError::Provider(
                loopal_error::ProviderError::StreamEnded,
            )),
        ],
        // Second LLM (auto-continue): re-issues the tool
        vec![
            Ok(StreamChunk::ToolUse {
                id: "tc-2".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        // Third LLM: final text after tool
        vec![
            Ok(StreamChunk::Text {
                text: "File contents retrieved.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "File contents retrieved.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    let _ = std::fs::remove_file(&tmp);
}

/// Empty stream errors are safe to replay, but exhaustion is a real Error.
#[tokio::test(start_paused = true)]
async fn test_empty_stream_error_retries_then_errors() {
    // Initial attempt plus all six retries return the intended transport
    // failure. Queue exhaustion is a fixture error and must not stand in for
    // the production retry-exhaustion behavior under test.
    let calls = (0..7)
        .map(|_| {
            vec![Err(loopal_error::LoopalError::Provider(
                loopal_error::ProviderError::StreamEnded,
            ))]
        })
        .collect();
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert!(output.result.contains("Stream ended unexpectedly"));
    assert_eq!(output.terminate_reason, TerminateReason::Error);
    assert_eq!(runner.turn_count, 0, "a failed turn is not completed");
}

/// EOF without Done and without output also exhausts to Error, not Goal.
#[tokio::test(start_paused = true)]
async fn test_eof_empty_stream_retries_then_errors() {
    // Script an explicit empty EOF for the initial attempt and all six
    // retries. An absent call would mean fixture underflow, not EOF.
    let calls: Vec<Vec<Result<StreamChunk, loopal_error::LoopalError>>> =
        (0..7).map(|_| Vec::new()).collect();
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert!(output.result.contains("Stream ended unexpectedly"));
    assert_eq!(output.terminate_reason, TerminateReason::Error);
    assert_eq!(runner.turn_count, 0, "a failed turn is not completed");
}
