use loopal_protocol::{AgentEventPayload, GoalTransitionReason, ThreadGoalStatus};
use loopal_tool_api::GoalSessionError;

use super::goal_session_support::{fixture, last_payload};

#[tokio::test]
async fn pause_then_resume_round_trip() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), None).await.unwrap();
    session
        .transition(ThreadGoalStatus::Paused, GoalTransitionReason::UserPaused)
        .await
        .unwrap();
    let g = session.snapshot().await.unwrap().unwrap();
    assert_eq!(g.status, ThreadGoalStatus::Paused);
    session
        .transition(ThreadGoalStatus::Active, GoalTransitionReason::UserResumed)
        .await
        .unwrap();
    let g = session.snapshot().await.unwrap().unwrap();
    assert_eq!(g.status, ThreadGoalStatus::Active);
}

#[tokio::test]
async fn extend_budget_atomically_bumps_and_reactivates() {
    let (_tmp, _store, emitter, session) = fixture();
    session.create("x".into(), Some(100)).await.unwrap();
    session.add_usage(100, 0).await.unwrap();
    assert_eq!(
        session.snapshot().await.unwrap().unwrap().status,
        ThreadGoalStatus::BudgetLimited
    );
    let extended = session.extend_budget(400).await.unwrap();
    assert_eq!(extended.token_budget, Some(500));
    assert_eq!(extended.status, ThreadGoalStatus::Active);
    assert!(matches!(
        last_payload(&emitter),
        AgentEventPayload::ThreadGoalUpdated {
            reason: GoalTransitionReason::UserExtendedBudget,
            ..
        }
    ));
}

#[tokio::test]
async fn extended_budget_resumes_accounting() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), Some(100)).await.unwrap();
    session.add_usage(100, 0).await.unwrap();
    session.extend_budget(400).await.unwrap();
    session.add_usage(100, 0).await.unwrap();
    let g = session.snapshot().await.unwrap().unwrap();
    assert_eq!(g.tokens_used, 200);
    assert_eq!(g.status, ThreadGoalStatus::Active);
}

#[tokio::test]
async fn extend_budget_rejects_when_not_budget_limited() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), Some(1_000)).await.unwrap();
    let err = session.extend_budget(500).await.unwrap_err();
    assert!(matches!(err, GoalSessionError::ModelStatusForbidden));
}

#[tokio::test]
async fn extend_budget_rejects_when_no_goal() {
    let (_tmp, _store, _emitter, session) = fixture();
    let err = session.extend_budget(500).await.unwrap_err();
    assert!(matches!(err, GoalSessionError::NotFound));
}

#[tokio::test]
async fn extend_budget_rejects_zero_amount() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into(), Some(100)).await.unwrap();
    session.add_usage(100, 0).await.unwrap();
    let err = session.extend_budget(0).await.unwrap_err();
    assert!(matches!(err, GoalSessionError::InvalidBudget));
}

#[tokio::test]
async fn set_session_id_redirects_reads_and_writes_to_new_session() {
    let (_tmp, store, _emitter, session) = fixture();
    session
        .create("alpha objective".into(), Some(1_000))
        .await
        .unwrap();
    assert!(store.load("sess").unwrap().is_some());

    session
        .set_session_id("sess-resumed".to_string())
        .await
        .expect("set_session_id");

    // The new session has no goal yet — snapshot should miss.
    assert!(session.snapshot().await.unwrap().is_none());

    // Creating after resume writes to the new directory.
    session.create("beta objective".into(), None).await.unwrap();
    let alpha = store.load("sess").unwrap().unwrap();
    let beta = store.load("sess-resumed").unwrap().unwrap();
    assert_eq!(alpha.objective, "alpha objective");
    assert_eq!(beta.objective, "beta objective");
}

#[tokio::test]
async fn set_session_id_rejects_empty() {
    let (_tmp, _store, _emitter, session) = fixture();
    let err = session
        .set_session_id(String::new())
        .await
        .expect_err("empty session id must be rejected");
    assert!(matches!(err, GoalSessionError::Storage(_)));
}

#[tokio::test]
async fn set_session_id_rejects_whitespace_only() {
    let (_tmp, _store, _emitter, session) = fixture();
    let err = session
        .set_session_id("   ".to_string())
        .await
        .expect_err("whitespace-only session id must be rejected");
    assert!(matches!(err, GoalSessionError::Storage(_)));
}
