//! Goal kickoff regression tests.
//!
//! Verify that `GoalCreate` / `GoalUserResume` / `GoalExtendBudget` control
//! commands cause the runner to inject a continuation envelope and call
//! the LLM, instead of staying parked in `wait_for_input`. The "LLM was
//! called" assertion rides on the LLM responding with `update_goal(complete)`
//! — only reachable if a turn actually fired.

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
            token_budget: None,
        })
        .await
        .unwrap();

    // If kickoff is broken, the runner stays in wait_for_input and never
    // calls update_goal — this wait will time out.
    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;

    let goal = session.snapshot().await.unwrap().expect("goal persisted");
    assert_eq!(goal.status, ThreadGoalStatus::Complete);
    drop(harness.control_tx);
}

#[tokio::test]
async fn goal_resume_via_control_triggers_turn() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("kickoff-resume").id);
    session
        .create("paused work".into(), None)
        .await
        .expect("create");
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

#[tokio::test]
async fn goal_extend_via_control_triggers_turn() {
    let fixture = TestFixture::new();
    let (_tmp, session, log) = make_goal_session(&fixture.test_session("kickoff-extend").id);
    session
        .create("crunched goal".into(), Some(100))
        .await
        .expect("create");
    session.add_usage(150, 0).await.expect("budget exhausted");

    let calls = vec![chunks::tool_turn(
        "ue1",
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
        .send(ControlCommand::GoalExtendBudget {
            additional_tokens: 1000,
        })
        .await
        .unwrap();

    wait_for_goal_reason(&log, GoalTransitionReason::ModelCompleted).await;
    drop(harness.control_tx);
}
