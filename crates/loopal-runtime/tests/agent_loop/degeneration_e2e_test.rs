use std::time::Duration;

use loopal_protocol::{
    DegenerationSignal, GateCloseReason, GoalTransitionReason, ThreadGoalStatus,
};
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;

use super::e2e_event_waiters::{
    wait_for_degeneration_event, wait_for_gate_change, wait_for_tool_error,
};
use super::goal_e2e_test::{make_goal_session, wait_for_goal_reason};

#[tokio::test]
async fn update_goal_infeasible_marks_goal_terminal_without_complete() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("degen-infeasible").id);
    session
        .create("structurally impossible".into())
        .await
        .unwrap();

    let calls = vec![chunks::tool_turn(
        "ui1",
        "update_goal",
        json!({"status": "infeasible"}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    wait_for_goal_reason(&log, GoalTransitionReason::ModelInfeasible).await;
    let goal = session.snapshot().await.unwrap().expect("persisted");
    assert_eq!(goal.status, ThreadGoalStatus::Infeasible);
    assert!(goal.status.is_terminal());
    assert!(!goal.status.participates_in_continuation());
    drop(harness.control_tx);
}

#[tokio::test]
async fn infeasible_goal_can_be_reopened_to_active() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("degen-reopen").id);
    session.create("provisional".into()).await.unwrap();

    let calls = vec![
        chunks::tool_turn("ui1", "update_goal", json!({"status": "infeasible"})),
        chunks::tool_turn("ui2", "update_goal", json!({"status": "active"})),
    ];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    wait_for_goal_reason(&log, GoalTransitionReason::ModelInfeasible).await;
    wait_for_goal_reason(&log, GoalTransitionReason::ModelReopened).await;
    let goal = session.snapshot().await.unwrap().expect("persisted");
    assert_eq!(goal.status, ThreadGoalStatus::Active);
    drop(harness.control_tx);
}

#[tokio::test]
async fn repeated_text_triggers_degeneration_event_keeps_goal_active() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("degen-repeat").id);
    session.create("long-horizon".into()).await.unwrap();

    let same = "16/100. Not complete. No action.";
    let calls = (0..8).map(|_| chunks::text_turn(same)).collect();
    let mut harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let summary = tokio::time::timeout(
        Duration::from_secs(10),
        wait_for_degeneration_event(&mut harness.event_rx),
    )
    .await
    .expect("expected DegenerationDetected before timeout");
    assert_eq!(summary.signal, DegenerationSignal::RepeatedText);
    assert!(summary.count >= 5);

    let gate = wait_for_gate_change(&mut harness.event_rx, false).await;
    assert_eq!(gate.closed_reason, Some(GateCloseReason::Degeneration));
    assert!(gate.wake_deadline.is_some());

    let goal = session.snapshot().await.unwrap().expect("persisted");
    assert_eq!(
        goal.status,
        ThreadGoalStatus::Active,
        "detector must never mutate goal status"
    );
    drop(harness.control_tx);
    drop(harness.mailbox_tx);
}

#[tokio::test]
async fn request_idle_rejects_duration_above_maximum() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("degen-idle-max").id);
    session.create("x".into()).await.unwrap();

    let calls = vec![chunks::tool_turn(
        "ri-max",
        "request_idle",
        json!({"reason": "too long", "max_idle_duration_secs": 99_999u64}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let recorded = wait_for_tool_error(&harness.recorded_messages).await;
    assert!(
        recorded.contains("exceeds the maximum"),
        "expected upper-bound rejection in tool_result, got {recorded:?}"
    );
    drop(harness.control_tx);
}

#[tokio::test]
async fn request_idle_rejects_missing_required_duration_arg() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("degen-idle-miss").id);
    session.create("x".into()).await.unwrap();

    let calls = vec![chunks::tool_turn(
        "ri-miss",
        "request_idle",
        json!({"reason": "no duration provided"}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let recorded = wait_for_tool_error(&harness.recorded_messages).await;
    assert!(
        recorded.contains("invalid arguments"),
        "expected parse-error rejection, got {recorded:?}"
    );
    drop(harness.control_tx);
}
