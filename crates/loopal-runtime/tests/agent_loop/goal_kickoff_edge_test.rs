//! Goal kickoff edge cases — negative paths (pause/clear should NOT trigger
//! a turn) and re-entrant scenarios (GoalCreate after a user-driven turn).

use std::time::Duration;

use loopal_protocol::{
    ControlCommand, Envelope, GoalTransitionReason, MessageSource, ThreadGoalStatus,
};
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;

use super::goal_e2e_test::{make_goal_session, wait_for_goal_reason};

#[tokio::test]
async fn goal_pause_does_not_trigger_turn() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("kickoff-pause").id);
    session
        .create("active work".into(), None)
        .await
        .expect("create");

    // No mock calls provisioned — any LLM invocation would yield an empty
    // stream, but more importantly we assert the agent *never* enters
    // Running by waiting on a paused-state poll loop without any turn
    // ever firing.
    let harness = HarnessBuilder::new()
        .calls(vec![])
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::GoalUserPause)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;
    let goal = session.snapshot().await.unwrap().expect("goal");
    assert_eq!(goal.status, ThreadGoalStatus::Paused);
    drop(harness.control_tx);
}

#[tokio::test]
async fn goal_clear_does_not_trigger_turn() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("kickoff-clear").id);
    session
        .create("work to clear".into(), None)
        .await
        .expect("create");

    let harness = HarnessBuilder::new()
        .calls(vec![])
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::GoalClear)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(session.snapshot().await.unwrap().is_none());
    drop(harness.control_tx);
}

#[tokio::test]
async fn goal_create_after_user_message_does_not_double_inject() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("kickoff-no-double").id);

    // First turn: user-driven text only. After it, store tail is Assistant.
    // Second control: GoalCreate — kickoff inject continuation, runner
    // calls LLM which completes the goal.
    let calls = vec![
        chunks::text_turn("first"),
        chunks::tool_turn("ud1", "update_goal", json!({"status": "complete"})),
    ];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    harness
        .mailbox_tx
        .send(Envelope::new(MessageSource::Human, "main", "kick"))
        .await
        .unwrap();

    // Wait for first turn to complete (Assistant tail) before sending control.
    tokio::time::sleep(Duration::from_millis(100)).await;

    harness
        .control_tx
        .send(ControlCommand::GoalCreate {
            objective: "second wave".into(),
            token_budget: None,
        })
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;
    drop(harness.control_tx);
}
