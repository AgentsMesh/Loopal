use loopal_provider_api::{ContinuationIntent, ContinuationReason, StopReason, StreamChunk};

use super::mock_provider::make_multi_runner_with_intents;

#[tokio::test]
async fn test_max_tokens_sets_pending_continuation() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "part 1".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "part 2".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let snapshot = intents.lock().unwrap().clone();
    assert_eq!(snapshot.len(), 2);
    assert!(snapshot[0].is_none(), "first call has no intent");
    assert!(
        matches!(
            snapshot[1],
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::MaxTokensWithoutTools
            })
        ),
        "second call must carry MaxTokensWithoutTools intent, got {:?}",
        snapshot[1]
    );
}

#[tokio::test]
async fn test_truncated_tools_sets_pending_continuation() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "Let me ".into(),
            }),
            Ok(StreamChunk::ToolUse {
                id: "tc-1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/tmp/truncated"}),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "read the file.".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let snapshot = intents.lock().unwrap().clone();
    assert!(
        matches!(
            snapshot.last().and_then(|i| i.as_ref()),
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::MaxTokensWithTools
            })
        ),
        "second call must carry MaxTokensWithTools intent, got {:?}",
        snapshot.last()
    );
}

#[tokio::test]
async fn test_continuation_marker_never_persisted_to_store() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "part 1".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::MaxTokens,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "part 2".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx, _intents) = make_multi_runner_with_intents(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let messages = runner.params.store.messages();
    let has_marker = messages.iter().any(|m| {
        m.content.iter().any(|b| matches!(
            b,
            loopal_message::ContentBlock::Text { text } if text.contains("[Continue from where you left off]")
        ))
    });
    assert!(
        !has_marker,
        "continuation marker must NEVER appear in persisted store"
    );
}

#[tokio::test]
async fn test_pause_turn_sets_continuation_pause_turn() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Text {
                text: "thinking…".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::PauseTurn,
            }),
        ],
        vec![
            Ok(StreamChunk::Text {
                text: "resumed".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let snapshot = intents.lock().unwrap().clone();
    assert!(
        matches!(
            snapshot.get(1).and_then(|i| i.as_ref()),
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::PauseTurn
            })
        ),
        "PauseTurn must produce ContinuationReason::PauseTurn intent, got {:?}",
        snapshot.get(1)
    );
}

#[tokio::test]
async fn test_stream_truncated_sets_continuation_stream_truncated() {
    // First call ends without a Done chunk → llm.rs detects stream truncation.
    // Some content must exist (assistant_text) so it isn't classified as
    // an empty / cancelled stream.
    let calls = vec![
        vec![Ok(StreamChunk::Text {
            text: "partial output".into(),
        })],
        vec![
            Ok(StreamChunk::Text {
                text: "completed".into(),
            }),
            Ok(StreamChunk::Done {
                stop_reason: StopReason::EndTurn,
            }),
        ],
    ];
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let snapshot = intents.lock().unwrap().clone();
    assert!(
        matches!(
            snapshot.get(1).and_then(|i| i.as_ref()),
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::StreamTruncated
            })
        ),
        "missing Done chunk must produce StreamTruncated intent, got {:?}",
        snapshot.get(1)
    );
}
