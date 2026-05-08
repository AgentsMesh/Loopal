//! `/goal` end-to-end edge cases — state-machine transitions driven via
//! `GoalRuntimeSession` directly because the runner is parked in the
//! hang-stream during these tests and would not service further control
//! commands until that turn ends.

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

#[tokio::test]
async fn extend_via_session_recovers_from_budget_limited_to_active() {
    let mut scenario = setup().await;

    run_goal(&mut scenario, Some("ship --budget=100")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    // Side-door budget exhaustion: add_usage is the same hook the runner
    // uses after each turn. Driving it directly avoids constructing a
    // mock-LLM Usage chunk stream while still funnelling through the
    // real GoalRuntimeSession state machine.
    scenario
        .session
        .add_usage(150, 0)
        .await
        .expect("add_usage should succeed");
    wait_for_status(&mut scenario, ThreadGoalStatus::BudgetLimited, TIMEOUT).await;
    drain_proxy(&mut scenario);
    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("[budget]"),
        "expected budget indicator, got:\n{frame}"
    );

    scenario
        .session
        .extend_budget(1500)
        .await
        .expect("extend budget");
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        drain_proxy(&mut scenario);
        let snap = scenario.session.snapshot().await.unwrap().expect("goal");
        if snap.status == ThreadGoalStatus::Active {
            assert_eq!(snap.token_budget, Some(1600));
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("did not return to Active after extend; current = {snap:?}");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("[active]"),
        "expected active indicator after extend, got:\n{frame}"
    );
}
