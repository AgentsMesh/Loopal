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

/// Empty stream error (Err on first chunk, no text) → no auto-continue, exit.
#[tokio::test]
async fn test_empty_stream_error_exits_without_continue() {
    let calls = vec![
        // Only one LLM call: immediate error
        vec![Err(loopal_error::LoopalError::Provider(
            loopal_error::ProviderError::StreamEnded,
        ))],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert!(output.result.is_empty());
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    // Only 1 LLM call — no retry for completely empty error
    assert_eq!(runner.turn_count, 1);
}

/// EOF without Done + empty response → exit (nothing to continue from).
#[tokio::test]
async fn test_eof_empty_stream_exits_without_continue() {
    let calls = vec![
        // Empty stream: no chunks at all, no Done
        vec![],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert!(output.result.is_empty());
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(runner.turn_count, 1);
}

/// Prior successful turn → second turn truncates → first turn's output preserved.
#[tokio::test]
async fn test_truncation_preserves_prior_turn_output() {
    let tmp = std::env::temp_dir().join(format!("la_prior_{}.txt", std::process::id()));
    std::fs::write(&tmp, "x").unwrap();
    let calls = vec![
        // First LLM: text + tool (normal)
        vec![
            Ok(StreamChunk::Text {
                text: "Step one done.".into(),
            }),
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        // Second LLM: truncated (text only, no Done)
        vec![Ok(StreamChunk::Text {
            text: "Starting step two...".into(),
        })],
        // Third LLM (auto-continue after truncation): final
        vec![
            Ok(StreamChunk::Text {
                text: "Step two complete.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "Step two complete.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    let _ = std::fs::remove_file(&tmp);
}

/// Truncation with only tool_uses (no text) → tools discarded, auto-continue.
#[tokio::test]
async fn test_truncation_tool_only_discards_and_continues() {
    let tmp = std::env::temp_dir().join(format!("la_toolonly_{}.txt", std::process::id()));
    std::fs::write(&tmp, "y").unwrap();
    let path_json = serde_json::json!({"file_path": tmp.to_str().unwrap()});
    let calls = vec![
        // First LLM: tool only, no Done (truncated)
        vec![Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: path_json.clone(),
        })],
        // Second LLM (auto-continue): tool + Done
        vec![
            Ok(StreamChunk::ToolUse { id: "tc-2".into(), name: "Read".into(), input: path_json }),
            Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn }),
        ],
        // Third LLM: final text
        vec![
            Ok(StreamChunk::Text { text: "Read complete.".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "Read complete.");
    let _ = std::fs::remove_file(&tmp);
}
