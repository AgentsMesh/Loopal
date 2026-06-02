use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use loopal_protocol::{AgentEventPayload, Envelope, MessageSource};
use loopal_runtime::agent_loop::governance::{Governance, PostTurnAction};
use loopal_runtime::agent_loop::turn_history::{TurnHistory, TurnRecord};
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use loopal_turn::{Turn, TurnTrigger};

use super::e2e_event_waiters::{wait_for_interrupted_event, wait_for_stream_event};
use super::goal_e2e_test::make_goal_session;

struct OnAfterTurnSpy {
    count: Arc<AtomicU32>,
    cancelled: Arc<AtomicU32>,
}

impl Governance for OnAfterTurnSpy {
    fn on_after_turn(&mut self, _record: &TurnRecord, _history: &TurnHistory) -> PostTurnAction {
        self.count.fetch_add(1, Ordering::Relaxed);
        PostTurnAction::None
    }
    fn on_turn_cancelled(&mut self) {
        self.cancelled.fetch_add(1, Ordering::Relaxed);
    }
}

// Control for interrupted_turn_skips_governance_after_turn: a normally completed
// turn DOES feed governance on_after_turn (count==1) and does NOT trigger
// on_turn_cancelled. Together the two pin the contract bidirectionally.
#[tokio::test]
async fn completed_turn_feeds_governance_after_turn() {
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("done")])
        .build()
        .await;
    let count = Arc::new(AtomicU32::new(0));
    let cancelled = Arc::new(AtomicU32::new(0));
    let mut runner = inner.runner;
    runner.governance.push(Box::new(OnAfterTurnSpy {
        count: count.clone(),
        cancelled: cancelled.clone(),
    }));
    runner.run().await.unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert_eq!(cancelled.load(Ordering::Relaxed), 0);
}

// A cancelled turn must NOT feed governance on_after_turn, but MUST trigger
// on_turn_cancelled (so LoopDetector resets its streak across the interrupt).
#[tokio::test]
async fn interrupted_turn_skips_governance_after_turn() {
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("streaming...")])
        .messages(vec![])
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .llm_chunk_delay(Duration::from_millis(80))
        .build()
        .await;
    let count = Arc::new(AtomicU32::new(0));
    let cancelled = Arc::new(AtomicU32::new(0));
    let mut runner = inner.runner;
    runner.governance.push(Box::new(OnAfterTurnSpy {
        count: count.clone(),
        cancelled: cancelled.clone(),
    }));
    let interrupt = inner.interrupt.clone();
    let mailbox = inner.mailbox_tx.clone();
    let mut event_rx = inner.event_rx;
    let task = tokio::spawn(async move { runner.run().await });

    mailbox
        .send(Envelope::new(MessageSource::Human, "main", "go"))
        .await
        .unwrap();
    wait_for_stream_event(&mut event_rx).await;
    interrupt.signal();
    wait_for_interrupted_event(&mut event_rx).await;

    drop(mailbox);
    drop(inner.mailbox_tx);
    drop(inner.control_tx);
    let _ = tokio::time::timeout(Duration::from_secs(1), task).await;

    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert!(cancelled.load(Ordering::Relaxed) >= 1);
}

fn goal_continuation_turn() -> Turn {
    Turn::new(TurnTrigger::GoalContinuation {
        envelope_id: "g1".into(),
        content: "keep going".into(),
    })
}

// A GoalContinuation turn whose goal changed before it started is skipped:
// emits ContinuationSkipped and is rewound (not executed). Regression for the
// round-7 skip path, which otherwise had no end-to-end coverage.
#[tokio::test]
async fn stale_continuation_turn_is_skipped() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("stale-skip").id);
    session.create("ongoing".into()).await.unwrap();
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("unused")])
        .messages(vec![])
        .goal_session(session.clone())
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .build()
        .await;
    let mut runner = inner.runner;
    // Stale id != the live goal's id → continuation_still_consistent is false.
    runner.last_continuation_goal_id = Some("stale-goal-id".into());
    runner.seed_test_turns(vec![goal_continuation_turn()]);
    let mut event_rx = inner.event_rx;
    let task = tokio::spawn(async move { runner.run().await });

    let mut saw_skipped = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, event_rx.recv()).await {
            Ok(Some(ev)) => {
                if matches!(ev.payload, AgentEventPayload::ContinuationSkipped { .. }) {
                    saw_skipped = true;
                    break;
                }
            }
            Ok(None) | Err(_) => break,
        }
    }
    drop(inner.mailbox_tx);
    drop(inner.control_tx);
    let _ = tokio::time::timeout(Duration::from_secs(1), task).await;

    assert!(
        saw_skipped,
        "stale continuation turn must emit ContinuationSkipped"
    );
}
