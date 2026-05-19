//! Tests for the runner-side post-conditions of the goal-kickoff control
//! path. Verifies that a continuation-injected wake-up carries a
//! System-tagged envelope (so observers can self-filter), and that a
//! human-typed message carries a Human-tagged envelope.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use loopal_protocol::{ControlCommand, GoalTransitionReason, MessageSource};
use loopal_runtime::agent_loop::WaitResult;
use loopal_runtime::agent_loop::governance::Governance;
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;

use super::goal_e2e_test::{make_goal_session, wait_for_goal_reason};

struct EnvelopeRecorder {
    count: Arc<AtomicU32>,
    last_source: Arc<Mutex<Option<MessageSource>>>,
}

impl Governance for EnvelopeRecorder {
    fn on_envelope_received(&mut self, source: &MessageSource) {
        self.count.fetch_add(1, Ordering::Relaxed);
        *self.last_source.lock().unwrap() = Some(source.clone());
    }
}

#[tokio::test]
async fn goal_create_via_control_returns_continuation_injected() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("kickoff-variant").id);

    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("ack")])
        .messages(vec![])
        .goal_session(session.clone())
        .build()
        .await;
    let mut runner = inner.runner;

    inner
        .control_tx
        .send(ControlCommand::GoalCreate {
            objective: "test variant".into(),
        })
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(
        matches!(result, Some(WaitResult::ContinuationInjected)),
        "kickoff path must return ContinuationInjected (not MessageAdded) so \
         the runner branch distinction stays observable — got {result:?}",
    );
    drop(inner.control_tx);
}

#[tokio::test]
async fn user_message_returns_message_added() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("kickoff-msg").id);

    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("ack")])
        .messages(vec![])
        .goal_session(session.clone())
        .build()
        .await;
    let mut runner = inner.runner;

    inner
        .mailbox_tx
        .send(loopal_protocol::Envelope::new(
            loopal_protocol::MessageSource::Human,
            "main",
            "hi",
        ))
        .await
        .unwrap();

    let result = runner.wait_for_input().await.unwrap();
    assert!(
        matches!(result, Some(WaitResult::MessageAdded)),
        "user-message path must keep returning MessageAdded — got {result:?}",
    );
    drop(inner.mailbox_tx);
}

#[tokio::test]
async fn kickoff_envelope_carries_system_source() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("kickoff-source").id);

    let count = Arc::new(AtomicU32::new(0));
    let last = Arc::new(Mutex::new(None));
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::tool_turn(
            "u1",
            "update_goal",
            json!({"status": "complete"}),
        )])
        .messages(vec![])
        .goal_session(session.clone())
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .build()
        .await;
    let mut runner = inner.runner;
    runner.governance.push(Box::new(EnvelopeRecorder {
        count: Arc::clone(&count),
        last_source: Arc::clone(&last),
    }));

    let runner_task = tokio::spawn(async move { runner.run().await });

    inner
        .control_tx
        .send(ControlCommand::GoalCreate {
            objective: "carry-source".into(),
        })
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;

    assert!(
        count.load(Ordering::Relaxed) >= 1,
        "kickoff path must notify observers at least once via ingest_message"
    );
    let recorded = last.lock().unwrap().clone();
    match recorded {
        Some(MessageSource::System(_)) => {}
        other => panic!(
            "kickoff envelope must be tagged System(_) so observers can \
             distinguish system continuation from real user input — got {other:?}"
        ),
    }

    drop(inner.control_tx);
    drop(inner.mailbox_tx);
    let _ = tokio::time::timeout(Duration::from_secs(1), runner_task).await;
}

#[tokio::test]
async fn user_message_envelope_carries_human_source() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("user-msg-source").id);

    let count = Arc::new(AtomicU32::new(0));
    let last = Arc::new(Mutex::new(None));
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("hi")])
        .messages(vec![])
        .goal_session(session.clone())
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .build()
        .await;
    let mut runner = inner.runner;
    runner.governance.push(Box::new(EnvelopeRecorder {
        count: Arc::clone(&count),
        last_source: Arc::clone(&last),
    }));

    let runner_task = tokio::spawn(async move { runner.run().await });

    inner
        .mailbox_tx
        .send(loopal_protocol::Envelope::new(
            MessageSource::Human,
            "main",
            "hello",
        ))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;

    let recorded = last.lock().unwrap().clone();
    assert!(
        matches!(recorded, Some(MessageSource::Human)),
        "user-message path must tag envelope as Human so LoopDetector resets — got {recorded:?}"
    );
    assert_eq!(
        count.load(Ordering::Relaxed),
        1,
        "user-message path must notify observers exactly once",
    );

    drop(inner.control_tx);
    drop(inner.mailbox_tx);
    let _ = tokio::time::timeout(Duration::from_secs(1), runner_task).await;
}
