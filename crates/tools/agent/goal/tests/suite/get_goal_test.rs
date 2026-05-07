use loopal_tool_api::Tool;
use loopal_tool_goal::GetGoalTool;
use serde_json::json;

use super::support::{FakeGoalSession, ctx_with_goal_session, ctx_without_goal_session};

#[tokio::test]
async fn returns_null_goal_when_session_empty() {
    let session = FakeGoalSession::empty();
    let ctx = ctx_with_goal_session(session);
    let result = GetGoalTool.execute(json!({}), &ctx).await.unwrap();
    assert!(!result.is_error);
    let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert!(parsed["goal"].is_null());
    assert!(parsed["remaining_tokens"].is_null());
}

#[tokio::test]
async fn returns_active_goal_with_remaining_tokens() {
    let session = FakeGoalSession::with_active("optimize hot path", Some(10_000));
    let ctx = ctx_with_goal_session(session);
    let result = GetGoalTool.execute(json!({}), &ctx).await.unwrap();
    assert!(!result.is_error);
    let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(parsed["goal"]["objective"], "optimize hot path");
    assert_eq!(parsed["goal"]["status"], "active");
    assert_eq!(parsed["remaining_tokens"], 10_000);
}

#[tokio::test]
async fn surfaces_disabled_when_no_goal_session_injected() {
    let ctx = ctx_without_goal_session();
    let result = GetGoalTool.execute(json!({}), &ctx).await.unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("disabled"));
}
