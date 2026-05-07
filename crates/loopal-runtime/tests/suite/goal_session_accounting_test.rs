use loopal_protocol::{AgentEventPayload, GoalTransitionReason, ThreadGoalStatus};
use loopal_runtime::goal::UsageOutcome;

use super::goal_session_support::fixture;

#[tokio::test]
async fn add_usage_accumulates_and_persists() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), Some(1_000)).await.unwrap();
    let outcome = session.add_usage(120, 50).await.unwrap();
    assert_eq!(outcome, UsageOutcome::Updated);
    let outcome = session.add_usage(80, 30).await.unwrap();
    assert_eq!(outcome, UsageOutcome::Updated);
    let g = session.snapshot().await.unwrap().unwrap();
    assert_eq!(g.tokens_used, 200);
    assert_eq!(g.time_used_ms, 80);
    assert_eq!(g.status, ThreadGoalStatus::Active);
}

#[tokio::test]
async fn add_usage_transitions_to_budget_limited_when_threshold_met() {
    let (_tmp, _store, emitter, session) = fixture();
    session.create("x".into(), Some(100)).await.unwrap();
    let outcome = session.add_usage(100, 0).await.unwrap();
    assert_eq!(outcome, UsageOutcome::BudgetExhausted);
    let g = session.snapshot().await.unwrap().unwrap();
    assert_eq!(g.status, ThreadGoalStatus::BudgetLimited);
    assert_eq!(g.tokens_used, 100);
    let last = emitter.events.lock().unwrap().last().cloned().unwrap();
    assert!(matches!(
        last,
        AgentEventPayload::ThreadGoalUpdated {
            reason: GoalTransitionReason::BudgetExhausted,
            ..
        }
    ));
}

#[tokio::test]
async fn add_usage_no_op_when_no_goal() {
    let (_tmp, _store, _emitter, session) = fixture();
    let outcome = session.add_usage(500, 100).await.unwrap();
    assert_eq!(outcome, UsageOutcome::NoOp);
    assert!(session.snapshot().await.unwrap().is_none());
}

#[tokio::test]
async fn add_usage_skipped_for_terminal_goal() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), Some(1_000)).await.unwrap();
    session
        .transition(
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
        )
        .await
        .unwrap();
    let outcome = session.add_usage(500, 100).await.unwrap();
    assert_eq!(outcome, UsageOutcome::NoOp);
    let g = session.snapshot().await.unwrap().unwrap();
    assert_eq!(g.tokens_used, 0);
    assert_eq!(g.time_used_ms, 0);
}

#[tokio::test]
async fn add_usage_zero_delta_is_noop() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), Some(100)).await.unwrap();
    let outcome = session.add_usage(0, 0).await.unwrap();
    assert_eq!(outcome, UsageOutcome::NoOp);
    let g = session.snapshot().await.unwrap().unwrap();
    assert_eq!(g.tokens_used, 0);
}

#[tokio::test]
async fn add_usage_skipped_for_paused_goal() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), Some(1_000)).await.unwrap();
    session
        .transition(ThreadGoalStatus::Paused, GoalTransitionReason::UserPaused)
        .await
        .unwrap();
    let outcome = session.add_usage(500, 100).await.unwrap();
    assert_eq!(outcome, UsageOutcome::NoOp);
}

#[tokio::test]
async fn add_usage_emits_usage_updated_on_each_increment() {
    let (_tmp, _store, emitter, session) = fixture();
    session.create("x".into(), Some(10_000)).await.unwrap();
    let baseline_count = emitter.events.lock().unwrap().len();
    session.add_usage(100, 50).await.unwrap();
    session.add_usage(200, 50).await.unwrap();
    let events = emitter.events.lock().unwrap();
    let usage_events: Vec<_> = events
        .iter()
        .skip(baseline_count)
        .filter(|p| {
            matches!(
                p,
                AgentEventPayload::ThreadGoalUpdated {
                    reason: GoalTransitionReason::UsageUpdated,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(
        usage_events.len(),
        2,
        "expected 2 UsageUpdated events, saw events {events:?}"
    );
}

#[tokio::test]
async fn add_usage_no_emit_on_noop() {
    let (_tmp, _store, emitter, session) = fixture();
    session.create("x".into(), Some(100)).await.unwrap();
    let baseline_count = emitter.events.lock().unwrap().len();
    session.add_usage(0, 0).await.unwrap();
    assert_eq!(emitter.events.lock().unwrap().len(), baseline_count);
}
