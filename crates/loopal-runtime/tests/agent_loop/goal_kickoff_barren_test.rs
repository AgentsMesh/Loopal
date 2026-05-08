//! Barren-continuation accounting tests for the goal-kickoff path.
//! Verifies which control commands reset `barren_continuation_count` and
//! which preserve it — locks the asymmetry the modes carry.

use std::sync::Arc;

use loopal_protocol::{ControlCommand, GoalTransitionReason, ThreadGoalStatus};
use loopal_test_support::{HarnessBuilder, TestFixture, chunks};

use super::goal_e2e_test::make_goal_session;

#[tokio::test]
async fn resume_via_control_resets_barren_continuation_count() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("kickoff-resume-reset").id);
    session.create("paused".into(), None).await.expect("create");
    session
        .transition(ThreadGoalStatus::Paused, GoalTransitionReason::UserPaused)
        .await
        .expect("pause");

    let inner = HarnessBuilder::new()
        .calls(vec![chunks::text_turn("ack")])
        .messages(vec![])
        .goal_session(session.clone())
        .build()
        .await;
    let mut runner = inner.runner;
    runner.barren_continuation_count = 5;

    inner
        .control_tx
        .send(ControlCommand::GoalUserResume)
        .await
        .unwrap();

    let _ = runner.wait_for_input().await.unwrap();
    assert_eq!(
        runner.barren_continuation_count, 0,
        "Resume must reset barren count for a fresh attempt window",
    );
    drop(inner.control_tx);
}

#[tokio::test]
async fn pause_via_control_preserves_barren_continuation_count() {
    let fixture = TestFixture::new();
    let (_tmp, session, _log) = make_goal_session(&fixture.test_session("kickoff-pause-keep").id);
    session.create("active".into(), None).await.expect("create");

    let inner = HarnessBuilder::new()
        .calls(vec![])
        .messages(vec![])
        .goal_session(session.clone())
        .build()
        .await;
    let mut runner = inner.runner;
    runner.barren_continuation_count = 3;
    let ctrl_tx = inner.control_tx.clone();
    ctrl_tx.send(ControlCommand::GoalUserPause).await.unwrap();
    drop(inner.control_tx);
    drop(inner.mailbox_tx);

    // Pause is not kickoff_eligible → wait_for_input continues looping
    // until channels close. With both senders dropped above, it returns
    // None.
    let _ = runner.wait_for_input().await.unwrap();
    assert_eq!(
        runner.barren_continuation_count, 3,
        "Pause must NOT touch barren count — only kickoff-eligible commands reset",
    );
    drop(ctrl_tx);
    let _ = Arc::strong_count(&session);
}
