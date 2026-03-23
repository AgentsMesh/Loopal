//! Edge-case tests for max_tokens auto-continuation.

use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{StopReason, StreamChunk};

use super::mock_provider::make_multi_runner;

/// Tool execution resets the continuation counter.
#[tokio::test]
async fn test_continuation_resets_after_tools() {
    let tmp = std::env::temp_dir().join(format!("la_acreset_{}.txt", std::process::id()));
    std::fs::write(&tmp, "x").unwrap();
    let calls = vec![
        // 1st LLM call: tool
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
        // 2nd LLM call (after tool): MaxTokens → should continue (counter was reset)
        vec![
            Ok(StreamChunk::Text {
                text: "after tool".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        // 3rd LLM call: normal end
        vec![
            Ok(StreamChunk::Text {
                text: " finished".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, " finished");
    let _ = std::fs::remove_file(&tmp);
}

/// AutoContinuation events carry correct values.
#[tokio::test]
async fn test_auto_continue_emits_event() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text { text: "a".into() }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text { text: "b".into() }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx) = make_multi_runner(calls);

    let events_handle = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(e) = event_rx.recv().await {
            events.push(e);
        }
        events
    });

    let _ = runner.run().await.unwrap();
    drop(runner); // Close event channel
    let events = events_handle.await.unwrap();

    let ac: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.payload, AgentEventPayload::AutoContinuation { .. }))
        .collect();
    assert_eq!(ac.len(), 1);
    match &ac[0].payload {
        AgentEventPayload::AutoContinuation {
            continuation,
            max_continuations,
        } => {
            assert_eq!(*continuation, 1);
            assert_eq!(*max_continuations, 3);
        }
        _ => unreachable!(),
    }
}
