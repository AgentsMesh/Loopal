use loopal_tool_api::Tool;
use serde_json::json;

use super::support::{
    FakeGoalSession, ctx_with_goal_session, ctx_without_goal_session, make_create_goal_tool,
};

#[tokio::test]
async fn creates_active_goal_with_objective_only() {
    let session = FakeGoalSession::empty();
    let ctx = ctx_with_goal_session(session.clone());
    let result = make_create_goal_tool()
        .execute(json!({"objective": "ship M0"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(parsed["goal"]["objective"], "ship M0");
    assert_eq!(parsed["goal"]["status"], "active");
    assert!(parsed["goal"]["token_budget"].is_null());
    assert!(session.goal.lock().unwrap().is_some());
}

#[tokio::test]
async fn creates_with_token_budget() {
    let session = FakeGoalSession::empty();
    let ctx = ctx_with_goal_session(session);
    let result = make_create_goal_tool()
        .execute(json!({"objective": "x", "token_budget": 5000}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(parsed["goal"]["token_budget"], 5000);
    assert_eq!(parsed["remaining_tokens"], 5000);
}

#[tokio::test]
async fn rejects_when_goal_already_exists() {
    let session = FakeGoalSession::with_active("existing", None);
    let ctx = ctx_with_goal_session(session);
    let result = make_create_goal_tool()
        .execute(json!({"objective": "second"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("already has a goal"));
}

#[tokio::test]
async fn rejects_empty_objective() {
    let session = FakeGoalSession::empty();
    let ctx = ctx_with_goal_session(session);
    let result = make_create_goal_tool()
        .execute(json!({"objective": "   "}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("non-empty"));
}

#[tokio::test]
async fn rejects_zero_budget() {
    let session = FakeGoalSession::empty();
    let ctx = ctx_with_goal_session(session);
    let result = make_create_goal_tool()
        .execute(json!({"objective": "x", "token_budget": 0}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("positive"));
}

#[tokio::test]
async fn surfaces_disabled_when_no_goal_session() {
    let ctx = ctx_without_goal_session();
    let result = make_create_goal_tool()
        .execute(json!({"objective": "x"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("disabled"));
}
