use std::time::Duration;

use loopal_protocol::{Envelope, MessageSource};
use loopal_test_support::{HarnessBuilder, chunks};

use super::e2e_event_waiters::wait_for_call_count;

// Regression: a stale `last_continuation_goal_id` + an inconsistent goal made
// the continuation gate fire for EVERY turn, silently swallowing real user
// input (TurnStarted→TurnEnded{Complete}, zero LlmCall). The gate must now
// only apply to GoalContinuation turns.
#[tokio::test]
async fn user_input_reaches_llm_despite_stale_continuation_goal() {
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("ok")])
        .messages(vec![])
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .build()
        .await;
    let recorded = inner.recorded_messages.clone();
    let mut runner = inner.runner;
    // No goal_session → continuation_still_consistent() returns false; the
    // stale id is what the buggy gate keyed on.
    runner.last_continuation_goal_id = Some("stale-goal".into());
    let task = tokio::spawn(async move { runner.run().await });

    inner
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "hi"))
        .await
        .unwrap();

    // Pre-fix this times out at 0 calls; post-fix the UserInput turn reaches
    // the LLM exactly as a non-continuation turn must.
    wait_for_call_count(&recorded, 1, Duration::from_secs(3)).await;

    drop(inner.mailbox_tx);
    drop(inner.control_tx);
    let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
}
