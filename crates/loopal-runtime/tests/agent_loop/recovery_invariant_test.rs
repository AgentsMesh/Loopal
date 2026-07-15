use std::sync::atomic::Ordering;

use loopal_provider_api::{ContinuationIntent, ContinuationReason, StopReason, StreamChunk};

use super::try_recover_helpers::{
    Outcome, context_overflow_err, make_runner, make_runner_with_intents, ok_done,
    seed_prior_completed_turn, server_block_err,
};

#[tokio::test]
async fn retry_after_continuation_failure_does_not_violate_invariant() {
    // Sequence (with prior history seeded so force_compact actually compacts):
    //   1) MaxTokens-with-tools → record assistant (no tools because truncated),
    //      set pending_continuation
    //   2) ContextOverflow during continuation → try_recover invokes
    //      force_compact, which calls the summarization LLM once
    //   3) compact succeeds → loop retries → next turn enters ReadyToCall with
    //      Assistant tail and RecoveryRetry intent
    //   4) Final success
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
        Outcome::Stream(ok_done()),
    ]);
    seed_prior_completed_turn(&mut runner);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "expected 4 LLM calls: truncated + overflow + compact-summary + retry-success; got {}",
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
        Outcome::Stream(ok_done()),
    ]);
    seed_prior_completed_turn(&mut runner);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });

    let _ = runner.run().await.unwrap();

    let snapshot = intents.lock().unwrap().clone();
    assert_eq!(snapshot.len(), 4);
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
        snapshot[2].is_none(),
        "third call (compact summarization) has no intent, got {:?}",
        snapshot[2]
    );
    assert!(
        matches!(
            snapshot[3],
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::RecoveryRetry
            })
        ),
        "fourth call (post-recovery retry) must carry RecoveryRetry intent, got {:?}",
        snapshot[3]
    );
}

#[tokio::test]
async fn server_block_recovery_also_re_primes_intent() {
    // Same invariant for ServerBlockError. server_block_err recovery uses
    // condense_server_blocks (no LLM call), so call count stays at 3.
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
