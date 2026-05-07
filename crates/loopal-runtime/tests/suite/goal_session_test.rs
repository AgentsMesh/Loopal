use loopal_protocol::{AgentEventPayload, GoalTransitionReason, ThreadGoal, ThreadGoalStatus};

use super::goal_session_support::{fixture, last_payload};

#[tokio::test]
async fn create_persists_and_emits_user_created() {
    let (_tmp, store, emitter, session) = fixture();
    let goal = session
        .create("ship M2".to_string(), Some(2_000))
        .await
        .unwrap();
    assert_eq!(goal.objective, "ship M2");
    assert_eq!(goal.status, ThreadGoalStatus::Active);
    assert_eq!(goal.token_budget, Some(2_000));

    let saved = store.load("sess").unwrap().unwrap();
    assert_eq!(saved.objective, "ship M2");

    match last_payload(&emitter) {
        AgentEventPayload::ThreadGoalUpdated { goal, reason } => {
            assert_eq!(goal.unwrap().objective, "ship M2");
            assert_eq!(reason, GoalTransitionReason::UserCreated);
        }
        _ => panic!("expected ThreadGoalUpdated"),
    }
}

#[tokio::test]
async fn create_rejects_duplicate() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("first".into(), None).await.unwrap();
    let err = session.create("second".into(), None).await.unwrap_err();
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::AlreadyExists
    ));
}

#[tokio::test]
async fn create_rejects_zero_budget() {
    let (_tmp, _store, _emitter, session) = fixture();
    let err = session
        .create("x".into(), Some(0))
        .await
        .expect_err("zero budget must fail");
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::InvalidBudget
    ));
}

#[tokio::test]
async fn transition_to_complete_via_model() {
    let (_tmp, _store, emitter, session) = fixture();
    session.create("x".into(), None).await.unwrap();
    let goal = session
        .transition(
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
        )
        .await
        .unwrap();
    assert_eq!(goal.status, ThreadGoalStatus::Complete);
    assert!(matches!(
        last_payload(&emitter),
        AgentEventPayload::ThreadGoalUpdated {
            reason: GoalTransitionReason::ModelCompleted,
            ..
        }
    ));
}

#[tokio::test]
async fn transition_rejects_illegal_via_status_machine() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), None).await.unwrap();
    session
        .transition(
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
        )
        .await
        .unwrap();
    let err = session
        .transition(ThreadGoalStatus::Active, GoalTransitionReason::UserResumed)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::ModelStatusForbidden
    ));
}

#[tokio::test]
async fn clear_removes_goal_and_emits() {
    let (_tmp, store, emitter, session) = fixture();
    session.create("x".into(), None).await.unwrap();
    let len_before_clear = emitter.events.lock().unwrap().len();
    session.clear().await.unwrap();
    assert!(store.load("sess").unwrap().is_none());
    assert!(emitter.events.lock().unwrap().len() > len_before_clear);
    assert!(matches!(
        last_payload(&emitter),
        AgentEventPayload::ThreadGoalUpdated {
            goal: None,
            reason: GoalTransitionReason::UserCleared,
        }
    ));
}

#[tokio::test]
async fn clear_when_no_goal_does_not_emit() {
    let (_tmp, _store, emitter, session) = fixture();
    session.clear().await.unwrap();
    assert!(emitter.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn snapshot_returns_none_for_empty_session() {
    let (_tmp, _store, _emitter, session) = fixture();
    assert!(session.snapshot().await.unwrap().is_none());
}

#[tokio::test]
async fn budget_exhausted_helper_consistent_after_load() {
    let (_tmp, store, _emitter, session) = fixture();
    let mut g = ThreadGoal::new("sess", "x");
    g.token_budget = Some(50);
    g.tokens_used = 50;
    store.save(&g).unwrap();
    let loaded = session.snapshot().await.unwrap().unwrap();
    assert!(loaded.budget_exhausted());
}

#[tokio::test]
async fn create_rejects_empty_objective() {
    let (_tmp, _store, _emitter, session) = fixture();
    let err = session.create(String::new(), None).await.unwrap_err();
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::ObjectiveTooLong { got: 0, .. }
    ));
}

#[tokio::test]
async fn create_rejects_overlong_objective() {
    let (_tmp, _store, _emitter, session) = fixture();
    let huge = "x".repeat(8_192);
    let err = session.create(huge, None).await.unwrap_err();
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::ObjectiveTooLong {
            max: 4096,
            got: 8192,
        }
    ));
}

#[tokio::test]
async fn create_overwrites_completed_goal() {
    let (_tmp, _store, _emitter, session) = fixture();
    let first = session.create("first".into(), None).await.unwrap();
    session
        .transition(
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
        )
        .await
        .unwrap();
    let second = session
        .create("second".into(), None)
        .await
        .expect("Complete goal should be overwritable");
    assert_ne!(first.goal_id, second.goal_id);
    assert_eq!(second.objective, "second");
    assert_eq!(second.status, ThreadGoalStatus::Active);
}

#[tokio::test]
async fn create_rejects_when_goal_paused() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("first".into(), None).await.unwrap();
    session
        .transition(ThreadGoalStatus::Paused, GoalTransitionReason::UserPaused)
        .await
        .unwrap();
    let err = session.create("second".into(), None).await.unwrap_err();
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::AlreadyExists
    ));
}

#[tokio::test]
async fn create_rejects_when_goal_budget_limited() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("first".into(), Some(50)).await.unwrap();
    session.add_usage(50, 0).await.unwrap();
    let err = session.create("second".into(), None).await.unwrap_err();
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::AlreadyExists
    ));
}
