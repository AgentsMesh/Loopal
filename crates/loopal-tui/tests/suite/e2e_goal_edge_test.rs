use std::time::Duration;

use loopal_protocol::{GoalTransitionReason, ThreadGoalStatus};

use super::e2e_goal_support::{drain_proxy, run_goal, setup, wait_for_status};

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::test]
async fn pause_then_resume_via_session_transitions_status() {
    let mut scenario = setup().await;
    run_goal(&mut scenario, Some("ship feature")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    scenario
        .session
        .transition(ThreadGoalStatus::Paused, GoalTransitionReason::UserPaused)
        .await
        .expect("pause transition");
    wait_for_status(&mut scenario, ThreadGoalStatus::Paused, TIMEOUT).await;
    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("paused"),
        "expected paused indicator, got:\n{frame}"
    );

    scenario
        .session
        .transition(ThreadGoalStatus::Active, GoalTransitionReason::UserResumed)
        .await
        .expect("resume transition");
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        drain_proxy(&mut scenario);
        let snap = scenario.session.snapshot().await.unwrap().expect("goal");
        if snap.status == ThreadGoalStatus::Active {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("did not return to Active after resume");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
