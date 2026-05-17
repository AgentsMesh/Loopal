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

#[derive(Default, Clone)]
pub(super) struct EventLog {
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
    pub(super) fn snapshot(&self) -> Vec<AgentEventPayload> {
        self.events.lock().unwrap().clone()
    }
}

pub(super) fn make_goal_session(session_id: &str) -> (TempDir, Arc<GoalRuntimeSession>, EventLog) {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(GoalStore::with_base_dir(tmp.path().to_path_buf()));
    let log = EventLog::default();
    let session = GoalRuntimeSession::new(session_id.to_string(), store, Box::new(log.clone()));
    (tmp, Arc::new(session), log)
}

pub(super) async fn wait_for_goal_reason(log: &EventLog, expected: GoalTransitionReason) {
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
        .create("ship the e2e".into())
        .await
        .expect("create goal");

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
async fn barren_continuations_auto_complete_goal() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("e2e-barren").id);
    session
        .create("idle work".into())
        .await
        .expect("create goal");

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
    assert_eq!(goal.status, ThreadGoalStatus::Complete);
    drop(mailbox_tx);
}
