//! Tests for the runner-side post-conditions of the goal-kickoff control
//! path. Distinguishes a continuation-injected wake-up from a user-input
//! wake-up via the `WaitResult` variant *and* via observer notification —
//! the two halves of the fix lock together against regressions.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use loopal_protocol::{ControlCommand, GoalTransitionReason, MessageSource};
use loopal_runtime::agent_loop::WaitResult;
use loopal_runtime::agent_loop::turn_observer::TurnObserver;
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;

use super::goal_e2e_test::{make_goal_session, wait_for_goal_reason};

/// Records `on_user_input` calls so tests can assert run_loop's variant
/// dispatch (MessageAdded ⇒ notify, ContinuationInjected ⇒ skip).
struct UserInputCounter {
    count: Arc<AtomicU32>,
}

impl TurnObserver for UserInputCounter {
    fn on_user_input(&mut self) {
        self.count.fetch_add(1, Ordering::Relaxed);
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
         run_loop skips on_user_input — got {result:?}",
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
async fn run_loop_kickoff_path_does_not_notify_user_input_observers() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("kickoff-no-notify").id);

    let count = Arc::new(AtomicU32::new(0));
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
    runner.observers.push(Box::new(UserInputCounter {
        count: Arc::clone(&count),
    }));

    let runner_task = tokio::spawn(async move { runner.run().await });

    inner
        .control_tx
        .send(ControlCommand::GoalCreate {
            objective: "no-notify".into(),
        })
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;

    assert_eq!(
        count.load(Ordering::Relaxed),
        0,
        "kickoff path must NOT call on_user_input — that hook is reserved \
         for fresh user input and resets cross-turn observer state",
    );

    drop(inner.control_tx);
    drop(inner.mailbox_tx);
    let _ = tokio::time::timeout(Duration::from_secs(1), runner_task).await;
}

#[tokio::test]
async fn run_loop_user_message_path_does_notify_user_input_observers() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("user-msg-notify").id);

    let count = Arc::new(AtomicU32::new(0));
    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("hi")])
        .messages(vec![])
        .goal_session(session.clone())
        .lifecycle(loopal_runtime::LifecycleMode::Persistent)
        .build()
        .await;
    let mut runner = inner.runner;
    runner.observers.push(Box::new(UserInputCounter {
        count: Arc::clone(&count),
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

    assert_eq!(
        count.load(Ordering::Relaxed),
        1,
        "user-message path must call on_user_input exactly once — pairs with \
         the kickoff-path negative test to lock both run_loop branches",
    );

    drop(inner.control_tx);
    drop(inner.mailbox_tx);
    let _ = tokio::time::timeout(Duration::from_secs(1), runner_task).await;
}
