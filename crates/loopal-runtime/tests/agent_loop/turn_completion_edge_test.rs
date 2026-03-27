//! Edge-case tests for turn completion behavior.
//! Covers error recovery, max_turns edge, and stream-error text preservation.

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

/// Tool execution no longer increments turn_count, so max_turns is not hit
/// inside execute_turn. The non-interactive agent exits after the turn completes.
#[tokio::test]
async fn test_max_turns_inside_execute_turn() {
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
    runner.params.config.max_turns = 1;
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    // Non-interactive: exits after first turn completes
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
    assert_eq!(runner.turn_count, 0);
    let _ = std::fs::remove_file(&tmp);
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
