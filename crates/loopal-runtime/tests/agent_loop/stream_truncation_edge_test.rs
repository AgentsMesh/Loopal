//! Edge-case integration tests for stream truncation.
//!
//! Covers EOF-without-Done scenarios and continuation exhaustion.

use loopal_error::TerminateReason;
use loopal_provider_api::{StopReason, StreamChunk};

use super::mock_provider::make_multi_runner;

/// EOF without Done + partial text → auto-continue → tool call → complete.
/// Reproduces the original sub-agent bug: proxy cuts SSE mid-response.
#[tokio::test]
async fn test_eof_text_then_tool_on_continue() {
    let tmp = std::env::temp_dir().join(format!("la_trunc_{}.txt", std::process::id()));
    std::fs::write(&tmp, "data").unwrap();
    let calls = vec![
        // First LLM: text only, EOF (no Done) — proxy cut
        vec![Ok(StreamChunk::Text {
            text: "Let me create the file.".into(),
        })],
        // Auto-continue: model sees partial text, issues tool
        vec![
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        // Final: text after tool execution
        vec![
            Ok(StreamChunk::Text {
                text: "File created.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "File created.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    let _ = std::fs::remove_file(&tmp);
}

/// Repeated EOF truncation exhausts max_auto_continuations → preserves last text.
#[tokio::test]
async fn test_repeated_truncation_exhausts_continuations() {
    // max_auto_continuations = 3: original + 3 retries = 4 LLM calls
    let calls = vec![
        vec![Ok(StreamChunk::Text {
            text: "attempt 1".into(),
        })],
        vec![Ok(StreamChunk::Text {
            text: "attempt 2".into(),
        })],
        vec![Ok(StreamChunk::Text {
            text: "attempt 3".into(),
        })],
        vec![Ok(StreamChunk::Text {
            text: "attempt 4".into(),
        })],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "attempt 4");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}

/// Prior successful turn → second turn truncates → auto-continue → final output.
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
        // Third LLM (auto-continue): final
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
        vec![Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: path_json.clone(),
        })],
        vec![
            Ok(StreamChunk::ToolUse {
                id: "tc-2".into(),
                name: "Read".into(),
                input: path_json,
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "Read complete.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "Read complete.");
    let _ = std::fs::remove_file(&tmp);
}
