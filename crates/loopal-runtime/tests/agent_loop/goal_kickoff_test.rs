use loopal_protocol::{ControlCommand, GoalTransitionReason, ThreadGoalStatus};
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};
use serde_json::json;

use super::goal_e2e_test::{make_goal_session, wait_for_goal_reason};

#[tokio::test]
async fn goal_create_via_control_triggers_turn() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("kickoff-create").id);

    let calls = vec![chunks::tool_turn(
        "uc1",
        "update_goal",
        json!({"status": "complete"}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::GoalCreate {
            objective: "ship the kickoff".into(),
        })
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;

    let goal = session.snapshot().await.unwrap().expect("goal persisted");
    assert_eq!(goal.status, ThreadGoalStatus::Complete);
    drop(harness.control_tx);
}

#[tokio::test]
async fn goal_resume_via_control_triggers_turn() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("kickoff-resume").id);
    session.create("paused work".into()).await.expect("create");
    session
        .transition(ThreadGoalStatus::Paused, GoalTransitionReason::UserPaused)
        .await
        .expect("pause");

    let calls = vec![chunks::tool_turn(
        "ur1",
        "update_goal",
        json!({"status": "complete"}),
    )];
    let harness = HarnessBuilder::new()
        .calls(calls)
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    harness
        .control_tx
        .send(ControlCommand::GoalUserResume)
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;
    drop(harness.control_tx);
}
