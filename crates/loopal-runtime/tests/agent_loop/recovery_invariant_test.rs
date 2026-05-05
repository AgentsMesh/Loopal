use std::sync::atomic::Ordering;

use loopal_provider_api::{ContinuationIntent, ContinuationReason, StopReason, StreamChunk};

use super::try_recover_helpers::{
    Outcome, context_overflow_err, make_runner, make_runner_with_intents, ok_done, server_block_err,
};

#[tokio::test]
async fn retry_after_continuation_failure_does_not_violate_invariant() {
    // Sequence:
    //   1) MaxTokens-with-tools → record assistant (no tools because truncated),
    //      set pending_continuation
    //   2) ContextOverflow during continuation → try_recover compacts and retries
    //      → next turn enters ReadyToCall with Assistant tail and no intent
    //      (turn_ctx is per-turn so pending_continuation reset to None)
    //   3) Final success
    let truncated_with_tools = vec![
        Ok(StreamChunk::Text {
            text: "partial ".into(),
        }),
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/x"}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::MaxTokens,
        }),
    ];
    let (mut runner, calls, mut rx) = make_runner(vec![
        Outcome::Stream(truncated_with_tools),
        Outcome::Err(context_overflow_err()),
        Outcome::Stream(ok_done()),
    ]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "expected 3 LLM calls: truncated + overflow + recovered success; got {}",
        calls.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn recovery_retry_call_carries_recovery_retry_intent() {
    // After retry, the new turn must re-prime pending_continuation with
    // RecoveryRetry so the LLM call still receives continuation context.
    // Without re-priming, supports_prefill=true models would receive an
    // Assistant tail with no continuation marker → prefill rejection.
    let truncated_with_tools = vec![
        Ok(StreamChunk::Text {
            text: "partial".into(),
        }),
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/x"}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::MaxTokens,
        }),
    ];
    let (mut runner, _calls, intents, mut rx) = make_runner_with_intents(vec![
        Outcome::Stream(truncated_with_tools),
        Outcome::Err(context_overflow_err()),
        Outcome::Stream(ok_done()),
    ]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let snapshot = intents.lock().unwrap().clone();
    assert_eq!(snapshot.len(), 3);
    assert!(snapshot[0].is_none(), "first call has no intent");
    assert!(
        matches!(
            snapshot[1],
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::MaxTokensWithTools
            })
        ),
        "second call (continuation) carries MaxTokensWithTools, got {:?}",
        snapshot[1]
    );
    assert!(
        matches!(
            snapshot[2],
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::RecoveryRetry
            })
        ),
        "third call (post-recovery retry) must carry RecoveryRetry intent, got {:?}",
        snapshot[2]
    );
}

#[tokio::test]
async fn server_block_recovery_also_re_primes_intent() {
    // Same invariant also applies to ServerBlockError recovery path.
    let truncated_with_tools = vec![
        Ok(StreamChunk::Text {
            text: "partial".into(),
        }),
        Ok(StreamChunk::ToolUse {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/x"}),
        }),
        Ok(StreamChunk::Done {
            stop_reason: StopReason::MaxTokens,
        }),
    ];
    let (mut runner, _calls, intents, mut rx) = make_runner_with_intents(vec![
        Outcome::Stream(truncated_with_tools),
        Outcome::Err(server_block_err()),
        Outcome::Stream(ok_done()),
    ]);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let snapshot = intents.lock().unwrap().clone();
    assert_eq!(snapshot.len(), 3);
    assert!(
        matches!(
            snapshot[2],
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::RecoveryRetry
            })
        ),
        "post-server-block-recovery call must carry RecoveryRetry, got {:?}",
        snapshot[2]
    );
}
