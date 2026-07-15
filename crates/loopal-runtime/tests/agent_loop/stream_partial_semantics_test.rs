use std::sync::atomic::Ordering;

use loopal_error::{LoopalError, ProviderError, TerminateReason};
use loopal_provider_api::{
    ContentBlock, ContinuationIntent, ContinuationReason, StopReason, StreamChunk,
};

use super::mock_provider::make_multi_runner_with_intents;
use super::try_recover_helpers::{
    Outcome, context_overflow_err, make_runner, ok_done, seed_prior_completed_turn,
    server_block_err,
};

fn done(text: &str) -> Vec<Result<StreamChunk, LoopalError>> {
    vec![
        Ok(StreamChunk::Text { text: text.into() }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::EndTurn,
        }),
    ]
}

fn assert_stream_continuation(intents: &[Option<ContinuationIntent>]) {
    assert!(matches!(
        intents.get(1).and_then(Option::as_ref),
        Some(ContinuationIntent::AutoContinue {
            reason: ContinuationReason::StreamTruncated
        })
    ));
}

#[tokio::test]
async fn thinking_only_eof_auto_continues() {
    let calls = vec![
        vec![Ok(StreamChunk::Thinking {
            text: "unfinished reasoning".into(),
        })],
        done("answer"),
    ];
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "answer");
    assert_stream_continuation(&intents.lock().unwrap());
}

#[tokio::test]
async fn server_blocks_eof_persists_pair_and_discards_orphan() {
    let calls = vec![
        vec![
            Ok(StreamChunk::Thinking {
                text: "signed reasoning".into(),
            }),
            Ok(StreamChunk::ThinkingSignature {
                signature: "reasoning-signature".into(),
            }),
            Ok(StreamChunk::ServerToolUse {
                id: "complete".into(),
                name: "web_search".into(),
                input: serde_json::json!({"query": "rust"}),
            }),
            Ok(StreamChunk::ServerToolResult {
                block_type: "web_search_tool_result".into(),
                tool_use_id: "complete".into(),
                content: serde_json::json!({"status": "done"}),
            }),
            Ok(StreamChunk::ServerToolUse {
                id: "orphan".into(),
                name: "web_search".into(),
                input: serde_json::json!({"query": "lost"}),
            }),
        ],
        done("answer"),
    ];
    let (mut runner, mut event_rx, intents) = make_multi_runner_with_intents(calls);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    runner.run().await.unwrap();
    assert_stream_continuation(&intents.lock().unwrap());
    let blocks: Vec<_> = runner
        .turns
        .view()
        .messages()
        .iter()
        .flat_map(|message| message.content.iter())
        .collect();
    assert!(
        blocks.iter().any(
            |block| matches!(block, ContentBlock::ServerToolUse { id, .. } if id == "complete")
        )
    );
    assert!(
        blocks
            .iter()
            .any(|block| matches!(block, ContentBlock::Thinking { .. }))
    );
    assert!(
        !blocks
            .iter()
            .any(|block| matches!(block, ContentBlock::ServerToolUse { id, .. } if id == "orphan"))
    );
}

#[tokio::test]
async fn in_stream_context_overflow_enters_compaction_recovery() {
    let (mut runner, calls, mut event_rx) = make_runner(vec![
        Outcome::Stream(vec![Err(context_overflow_err())]),
        Outcome::Stream(ok_done()),
        Outcome::Stream(done("recovered")),
    ]);
    seed_prior_completed_turn(&mut runner);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "recovered");
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn in_stream_server_block_error_enters_condense_recovery() {
    let (mut runner, calls, mut event_rx) = make_runner(vec![
        Outcome::Stream(vec![Err(server_block_err())]),
        Outcome::Stream(done("recovered")),
    ]);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.result, "recovered");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn in_stream_fatal_error_is_not_a_goal_completion() {
    let error = LoopalError::Provider(ProviderError::Api {
        status: 400,
        message: "invalid request".into(),
    });
    let (mut runner, _calls, mut event_rx) = make_runner(vec![Outcome::Stream(vec![Err(error)])]);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let output = runner.run().await.unwrap();
    assert_eq!(output.terminate_reason, TerminateReason::Error);
    assert!(output.result.contains("invalid request"));
}
