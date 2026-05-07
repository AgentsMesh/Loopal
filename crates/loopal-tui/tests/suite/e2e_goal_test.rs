//! `/goal` end-to-end tests.
//!
//! Cover the path TUI handler -> control channel -> agent runtime ->
//! `GoalRuntimeSession` -> `ThreadGoalUpdated` event -> ViewState
//! mutator -> status-bar render. The runtime is real; only the LLM
//! provider is mocked. Goal events ride a proxy `mpsc` because the goal
//! session is constructed before the wiring channel exists; the test
//! drains the proxy into `App::dispatch_event` to mirror what the
//! production frontend forwarder does.

use std::time::Duration;

use loopal_protocol::{AgentStateSnapshot, ThreadGoalStatus};
use loopal_tui::command::CommandEffect;
use loopal_tui::view_client::ViewClient;
use loopal_view_state::{SessionViewState, ViewSnapshot};

use super::e2e_goal_support::{drain_proxy, last_system_message, run_goal, setup, wait_for_status};

const TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::test]
async fn create_command_drives_runtime_and_renders_status_bar() {
    let mut scenario = setup().await;

    let effect = run_goal(&mut scenario, Some("ship feature")).await;
    assert!(
        matches!(&effect, CommandEffect::Reply(s) if s.contains("ship feature")),
        "expected Reply with objective, got {:?}",
        std::mem::discriminant(&effect)
    );
    assert!(last_system_message(&scenario.harness.app).contains("ship feature"));

    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("ship feature") && frame.contains("active"),
        "status bar missing goal indicator:\n{frame}"
    );
}

#[tokio::test]
async fn pause_then_resume_cycle_through_runtime() {
    let mut scenario = setup().await;
    run_goal(&mut scenario, Some("ship feature")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    run_goal(&mut scenario, Some("pause")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Paused, TIMEOUT).await;
    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("paused"),
        "expected paused indicator, got:\n{frame}"
    );

    run_goal(&mut scenario, Some("resume")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;
}

#[tokio::test]
async fn clear_command_removes_goal_from_status_bar() {
    let mut scenario = setup().await;
    run_goal(&mut scenario, Some("ship feature")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;
    assert!(scenario.harness.render_text().contains("[active]"));

    run_goal(&mut scenario, Some("clear")).await;
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        drain_proxy(&mut scenario);
        if scenario.session.snapshot().await.unwrap().is_none() {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("goal still present after clear");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // The Reply text "Goal set: ..." stays in the conversation history, so
    // we can't ask "frame must not contain ship feature". The status bar
    // indicator is what tracks goal state — its lifecycle bracket label
    // (active/paused/done/budget) disappears when the goal is cleared.
    let frame = scenario.harness.render_text();
    assert!(
        !frame.contains("[active]")
            && !frame.contains("[paused]")
            && !frame.contains("[done]")
            && !frame.contains("[budget]"),
        "status indicator still present:\n{frame}"
    );
}

#[tokio::test]
async fn empty_arg_replies_with_usage_and_does_not_create_goal() {
    let mut scenario = setup().await;
    let effect = run_goal(&mut scenario, None).await;

    let usage = match effect {
        CommandEffect::Reply(s) => s,
        other => panic!(
            "expected Reply for empty arg, got {:?}",
            std::mem::discriminant(&other)
        ),
    };
    assert!(usage.contains("/goal usage"));
    assert!(last_system_message(&scenario.harness.app).contains("/goal usage"));

    tokio::time::sleep(Duration::from_millis(50)).await;
    drain_proxy(&mut scenario);
    assert!(
        scenario.session.snapshot().await.unwrap().is_none(),
        "no goal should have been created"
    );
}

#[tokio::test]
async fn extend_recovers_from_budget_limited_to_active() {
    let mut scenario = setup().await;

    // Create with a tiny budget so a synthetic add_usage can blow it.
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
    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("[budget]"),
        "expected budget indicator, got:\n{frame}"
    );

    // Extend bumps budget AND transitions back to Active in one atomic
    // step. The full pipeline (TUI parse → control → runtime → emit →
    // dispatch → render) must reflect both halves.
    run_goal(&mut scenario, Some("extend 1500")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;
    let snap = scenario.session.snapshot().await.unwrap().unwrap();
    assert_eq!(snap.token_budget, Some(1600));
    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("[active]"),
        "expected active indicator after extend, got:\n{frame}"
    );
}

#[tokio::test]
async fn reconnecting_view_client_sees_existing_goal_via_snapshot() {
    let mut scenario = setup().await;

    // Establish a live goal through the normal command path.
    run_goal(&mut scenario, Some("ship feature")).await;
    wait_for_status(&mut scenario, ThreadGoalStatus::Active, TIMEOUT).await;

    // Reproduce the production reconnect hop:
    //   AgentShared.snapshot_state() -> AgentStateSnapshot
    //   SessionViewState::from_snapshot()
    //   ViewSnapshot { state, rev }
    //   ViewClient::from_snapshot()
    // ThreadGoalUpdated events fired before reconnect aren't replayed,
    // so the goal must survive purely on the snapshot wire payload.
    let live_goal = scenario
        .session
        .snapshot()
        .await
        .unwrap()
        .expect("goal exists");
    let agent_snap = AgentStateSnapshot {
        tasks: vec![],
        crons: vec![],
        bg_tasks: vec![],
        thread_goal: Some(live_goal.clone()),
    };
    let rebuilt = SessionViewState::from_snapshot("main", agent_snap);
    let view_snap = ViewSnapshot {
        state: rebuilt,
        rev: 1,
    };

    // Replace the live ViewClient with one seeded purely from the
    // snapshot — the rendered status bar must still show the goal.
    scenario
        .harness
        .app
        .view_clients
        .insert("main".into(), ViewClient::from_snapshot("main", view_snap));
    let frame = scenario.harness.render_text();
    assert!(
        frame.contains("ship feature") && frame.contains("[active]"),
        "reconnected client missing goal indicator:\n{frame}"
    );
}
