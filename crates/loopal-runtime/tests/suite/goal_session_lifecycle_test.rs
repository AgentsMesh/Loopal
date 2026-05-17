use loopal_protocol::{GoalTransitionReason, ThreadGoalStatus};
use loopal_tool_api::GoalSessionError;

use super::goal_session_support::fixture;

#[tokio::test]
async fn pause_then_resume_round_trip() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("x".into()).await.unwrap();
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
async fn set_session_id_redirects_reads_and_writes_to_new_session() {
    let (_tmp, store, _emitter, session) = fixture();
    session.create("alpha objective".into()).await.unwrap();
    assert!(store.load("sess").unwrap().is_some());

    session
        .set_session_id("sess-resumed".to_string())
        .await
        .expect("set_session_id");

    assert!(session.snapshot().await.unwrap().is_none());

    session.create("beta objective".into()).await.unwrap();
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
