//! Tests for max_tokens auto-continuation in execute_turn.

use loopagent_types::error::TerminateReason;
use loopagent_types::provider::{StopReason, StreamChunk};

use super::mock_provider::make_multi_runner;

/// MaxTokens on text-only → auto-continue → EndTurn completes.
#[tokio::test]
async fn test_auto_continue_text_only() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text { text: "part 1".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::MaxTokens }),
        ],
        vec![
            Ok(StreamChunk::Text { text: " part 2".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, " part 2");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}

/// 4 consecutive MaxTokens → stops at limit (3 continuations).
#[tokio::test]
async fn test_auto_continue_limit() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text { text: "a".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::MaxTokens }),
        ],
        vec![
            Ok(StreamChunk::Text { text: "b".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::MaxTokens }),
        ],
        vec![
            Ok(StreamChunk::Text { text: "c".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::MaxTokens }),
        ],
        // 4th call: still MaxTokens but limit = 3 reached
        vec![
            Ok(StreamChunk::Text { text: "d".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::MaxTokens }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "d");
}

/// MaxTokens + ToolUse → tools discarded, triggers continuation.
#[tokio::test]
async fn test_max_tokens_with_tools_discards() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text { text: "Let me ".into() }),
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(), name: "Read".into(),
                input: serde_json::json!({"file_path": "/tmp/truncated"}),
            }),
            Ok(StreamChunk::Done { stop_reason: StopReason::MaxTokens }),
        ],
        vec![
            Ok(StreamChunk::Text { text: "read the file.".into() }),
            Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "read the file.");
    // Tool was discarded — turn_count stays 0
    assert_eq!(runner.turn_count, 0);
}

/// EndTurn → no continuation, normal behavior.
#[tokio::test]
async fn test_end_turn_no_continuation() {
    let calls = vec![vec![
        Ok(StreamChunk::Text { text: "done".into() }),
        Ok(StreamChunk::Done { stop_reason: StopReason::EndTurn }),
    ]];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "done");
    assert_eq!(output.terminate_reason, TerminateReason::Goal);
}
