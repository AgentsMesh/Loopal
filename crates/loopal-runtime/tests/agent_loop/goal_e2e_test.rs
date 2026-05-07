//! End-to-end goal lifecycle tests through the full agent loop.
//!
//! Drives a real `AgentLoopRunner` with a `MultiCallProvider` and a
//! `GoalRuntimeSession` plumbed through `HarnessBuilder`. Verifies the
//! invariants the unit tests cannot — that the runner integration points
//! actually trigger when the loop runs end-to-end.

use std::sync::Arc;
use std::time::Duration;

use loopal_protocol::{
    AgentEventPayload, Envelope, GoalTransitionReason, MessageSource, ThreadGoalStatus,
};
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::goal::GoalRuntimeSession;
use loopal_storage::GoalStore;
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;
use tempfile::TempDir;

/// Captures `ThreadGoalUpdated` events emitted by `GoalRuntimeSession` so
/// tests can assert on the broadcast surface independently of the frontend
/// event channel (the test fixture wires those separately).
#[derive(Default, Clone)]
struct EventLog {
    events: Arc<std::sync::Mutex<Vec<AgentEventPayload>>>,
}

#[async_trait::async_trait]
impl EventEmitter for EventLog {
    async fn emit(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        self.events.lock().unwrap().push(payload);
        Ok(())
    }
}

impl EventLog {
    fn snapshot(&self) -> Vec<AgentEventPayload> {
        self.events.lock().unwrap().clone()
    }
}

fn make_goal_session(session_id: &str) -> (TempDir, Arc<GoalRuntimeSession>, EventLog) {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(GoalStore::with_base_dir(tmp.path().to_path_buf()));
    let log = EventLog::default();
    let session = GoalRuntimeSession::new(session_id.to_string(), store, Box::new(log.clone()));
    (tmp, Arc::new(session), log)
}

/// Poll `EventLog` until a `ThreadGoalUpdated` with the expected reason
/// shows up, or panic on timeout. Avoids racing with the runner — once
/// the predicate sees the event, the underlying mutation is durably
/// persisted.
async fn wait_for_goal_reason(log: &EventLog, expected: GoalTransitionReason) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if log.snapshot().iter().any(|p| {
            matches!(
                p,
                AgentEventPayload::ThreadGoalUpdated { reason, .. } if *reason == expected
            )
        }) {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "timed out waiting for ThreadGoalUpdated({expected:?}); saw {:?}",
                log.snapshot()
            );
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn happy_path_user_creates_then_model_completes() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("e2e").id);
    session
        .create("ship the e2e".into(), None)
        .await
        .expect("create goal");

    // Turn 1: user-driven, text only.
    // Idle → continuation envelope auto-injected.
    // Turn 2: model calls update_goal(complete), Done.
    let calls = vec![
        chunks::text_turn("acknowledged"),
        chunks::tool_turn("uc1", "update_goal", json!({"status": "complete"})),
    ];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let mailbox_tx = harness.mailbox_tx;
    mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "begin"))
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;

    let goal = session.snapshot().await.unwrap().expect("goal persisted");
    assert_eq!(goal.status, ThreadGoalStatus::Complete);
    drop(mailbox_tx);
}

#[tokio::test]
async fn budget_exhaustion_transitions_and_emits_event() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("e2e-budget").id);
    session
        .create("crunch the budget".into(), Some(100))
        .await
        .expect("create goal");

    // Turn 1: user-driven; LLM text + Usage(150,30) — exceeds budget=100.
    // Turn 2: continuation with budget_limit prompt; LLM closes out.
    let turn1 = vec![
        chunks::text("starting"),
        chunks::usage(150, 30),
        chunks::done(),
    ];
    let turn2 = chunks::text_turn("wrapping up");
    let harness = HarnessBuilder::new()
        .calls(vec![turn1, turn2])
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let mailbox_tx = harness.mailbox_tx;
    mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "go"))
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::BudgetExhausted).await;

    let goal = session.snapshot().await.unwrap().expect("goal persisted");
    assert_eq!(goal.status, ThreadGoalStatus::BudgetLimited);
    assert!(
        goal.tokens_used >= 100,
        "tokens_used = {}",
        goal.tokens_used
    );
    drop(mailbox_tx);
}

#[tokio::test]
async fn barren_continuations_demote_to_budget_limited() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("e2e-barren").id);
    session
        .create("idle work".into(), None)
        .await
        .expect("create goal");

    // Turn 1: user-driven text-only (productive=false but not continuation).
    // Turn 2 (continuation 1): text-only → barren_count = 1.
    // Turn 3 (continuation 2): text-only → barren_count = 2.
    // Next idle: barren_count >= max → demote to BudgetLimited, no turn 4.
    let calls = vec![
        chunks::text_turn("hello"),
        chunks::text_turn("still working"),
        chunks::text_turn("still working"),
    ];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let mailbox_tx = harness.mailbox_tx;
    mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "kick off"))
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::BarrenContinuation).await;

    let goal = session.snapshot().await.unwrap().expect("goal persisted");
    assert_eq!(goal.status, ThreadGoalStatus::BudgetLimited);
    drop(mailbox_tx);
}
