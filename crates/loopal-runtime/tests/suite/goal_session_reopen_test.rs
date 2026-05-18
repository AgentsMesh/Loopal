use loopal_protocol::{AgentEventPayload, GoalTransitionReason, ThreadGoalStatus};

use super::goal_session_support::{fixture, last_payload};

#[tokio::test]
async fn transition_complete_to_active_via_model_reopened_succeeds() {
    let (_tmp, store, emitter, session) = fixture();
    session.create("ship".into()).await.unwrap();
    session
        .transition(
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
        )
        .await
        .unwrap();

    let reopened = session
        .transition(
            ThreadGoalStatus::Active,
            GoalTransitionReason::ModelReopened,
        )
        .await
        .unwrap();

    assert_eq!(reopened.status, ThreadGoalStatus::Active);
    assert_eq!(reopened.objective, "ship");
    let saved = store.load("sess").unwrap().unwrap();
    assert_eq!(saved.status, ThreadGoalStatus::Active);
    assert_eq!(saved.goal_id, reopened.goal_id);
    assert!(matches!(
        last_payload(&emitter),
        AgentEventPayload::ThreadGoalUpdated {
            reason: GoalTransitionReason::ModelReopened,
            ..
        }
    ));
}

#[tokio::test]
async fn transition_complete_to_active_via_user_reopened_succeeds() {
    let (_tmp, _store, emitter, session) = fixture();
    session.create("ship".into()).await.unwrap();
    session
        .transition(
            ThreadGoalStatus::Complete,
            GoalTransitionReason::UserCompleted,
        )
        .await
        .unwrap();

    let reopened = session
        .transition(ThreadGoalStatus::Active, GoalTransitionReason::UserReopened)
        .await
        .unwrap();

    assert_eq!(reopened.status, ThreadGoalStatus::Active);
    assert!(matches!(
        last_payload(&emitter),
        AgentEventPayload::ThreadGoalUpdated {
            reason: GoalTransitionReason::UserReopened,
            ..
        }
    ));
}

#[tokio::test]
async fn reopen_when_active_is_rejected() {
    let (_tmp, _store, _emitter, session) = fixture();
    session.create("ship".into()).await.unwrap();
    let err = session
        .transition(
            ThreadGoalStatus::Active,
            GoalTransitionReason::ModelReopened,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        loopal_tool_api::GoalSessionError::ModelStatusForbidden
    ));
}

#[tokio::test]
async fn reopen_when_no_goal_returns_not_found() {
    let (_tmp, _store, _emitter, session) = fixture();
    let err = session
        .transition(
            ThreadGoalStatus::Active,
            GoalTransitionReason::ModelReopened,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, loopal_tool_api::GoalSessionError::NotFound));
}

#[tokio::test]
async fn adapter_reopen_by_model_routes_through_transition() {
    use std::sync::Arc;

    use loopal_runtime::GoalSessionToolAdapter;
    use loopal_tool_api::GoalSession;

    let (_tmp, _store, emitter, session) = fixture();
    let session = Arc::new(session);
    session.create("ship".into()).await.unwrap();
    session
        .transition(
            ThreadGoalStatus::Complete,
            GoalTransitionReason::ModelCompleted,
        )
        .await
        .unwrap();

    let adapter = GoalSessionToolAdapter::new(Arc::clone(&session));
    let reopened = adapter.reopen_by_model().await.unwrap();

    assert_eq!(reopened.status, ThreadGoalStatus::Active);
    assert!(matches!(
        last_payload(&emitter),
        AgentEventPayload::ThreadGoalUpdated {
            reason: GoalTransitionReason::ModelReopened,
            ..
        }
    ));
}
