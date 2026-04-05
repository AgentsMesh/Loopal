//! Edge-case tests for turn completion behavior.
//! Covers error recovery and stream-error text preservation.

use loopal_error::{LoopalError, TerminateReason};
use loopal_provider_api::{StopReason, StreamChunk};

use super::turn_completion_test::make_multi_runner;

/// First turn succeeds, second turn errors -> result preserves first output.
#[tokio::test]
async fn test_error_preserves_prior_output() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "first output".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
        // Second LLM call is attempted on next iteration but won't happen
        // because non-interactive exits after first turn with no tool calls.
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "first output");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}

/// Non-interactive agent exits after the turn completes.
#[tokio::test]
async fn test_non_interactive_exits_after_tool_turn() {
    let tmp = std::env::temp_dir().join(format!("la_mt_{}.txt", std::process::id()));
    std::fs::write(&tmp, "y").unwrap();
    let calls = vec![vec![
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": tmp.to_str().unwrap()}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // Non-interactive: exits after first turn completes
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(runner.turn_count, 1);
    let _ = std::fs::remove_file(&tmp);
}

/// Regression: tool call with text -> next LLM returns empty (no text, no
/// tools) -> output must preserve text from the earlier iteration.
/// This was the root cause of sub-agents returning output_len=0 to parents:
/// the normal exit path in `execute_turn_inner` used `result.assistant_text`
/// (empty) instead of `last_text` (accumulated).
#[tokio::test]
async fn test_empty_final_response_preserves_last_text() {
    let tmp = std::env::temp_dir().join(format!("la_ef_{}.txt", std::process::id()));
    std::fs::write(&tmp, "content").unwrap();
    let calls = vec![
        // First LLM call: text + tool
        vec![
            Ok(StreamChunk::Text {
                text: "Let me check the file.".into(),
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
        // Second LLM call: empty response (no text, no tools)
        vec![Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        })],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "Let me check the file.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    let _ = std::fs::remove_file(&tmp);
}

/// Stream truncation (EOF without Done) with partial text → auto-continue →
/// second LLM call completes normally with a tool → tool executes.
#[tokio::test]
async fn test_stream_truncation_continues_from_partial() {
    let tmp = std::env::temp_dir().join(format!("la_trunc_{}.txt", std::process::id()));
    std::fs::write(&tmp, "data").unwrap();
    let calls = vec![
        // First LLM call: text only, then EOF (no Done) — simulates proxy cut.
        vec![Ok(StreamChunk::Text {
            text: "Let me create the file.".into(),
        })],
        // Second LLM call (auto-continue): model sees partial text, completes with tool.
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
        // Third LLM call: final text after tool execution.
        vec![
            Ok(StreamChunk::Text {
                text: "File created.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "File created.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    let _ = std::fs::remove_file(&tmp);
}

/// Repeated stream truncation exhausts max_auto_continuations → preserves last text.
#[tokio::test]
async fn test_stream_truncation_max_continuations() {
    // Default max_auto_continuations = 3, so we need 4 truncated calls:
    // 1 original + 3 continuations = 4 calls total, all truncated.
    let calls = vec![
        vec![Ok(StreamChunk::Text { text: "attempt 1".into() })],
        vec![Ok(StreamChunk::Text { text: "attempt 2".into() })],
        vec![Ok(StreamChunk::Text { text: "attempt 3".into() })],
        vec![Ok(StreamChunk::Text { text: "attempt 4".into() })],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // After 3 continuations exhausted, last text should be preserved.
    assert_eq!(output.result, "attempt 4");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}

/// Regression test: tool call with text -> next LLM call stream error ->
/// output preserves the text from the successful iteration (not empty).
/// This was the root cause of sub-agents returning empty results.
#[tokio::test]
async fn test_stream_error_after_tool_preserves_last_text() {
    let tmp = std::env::temp_dir().join(format!("la_se_{}.txt", std::process::id()));
    std::fs::write(&tmp, "data").unwrap();
    let calls = vec![
        // First LLM call: text + tool
        vec![
            Ok(StreamChunk::Text {
                text: "I will read the file.".into(),
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
        // Second LLM call: stream error (simulates 502/connection reset)
        vec![Err(LoopalError::Provider(
            loopal_error::ProviderError::StreamEnded,
        ))],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls, false);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // The key assertion: even though the second LLM call had a stream error,
    // the output preserves "I will read the file." from the first iteration.
    assert_eq!(output.result, "I will read the file.");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);

    let _ = std::fs::remove_file(&tmp);
}
