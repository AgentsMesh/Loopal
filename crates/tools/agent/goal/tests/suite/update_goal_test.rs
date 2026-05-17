use loopal_tool_api::Tool;
use serde_json::json;

use super::support::{
    FakeGoalSession, ctx_with_goal_session, ctx_without_goal_session, make_update_goal_tool,
};

#[tokio::test]
async fn marks_active_goal_complete() {
    let session = FakeGoalSession::with_active("ship");
    let ctx = ctx_with_goal_session(session.clone());
    let result = make_update_goal_tool()
        .execute(json!({"status": "complete"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    let parsed: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(parsed["goal"]["status"], "complete");
    assert_eq!(
        session.goal.lock().unwrap().as_ref().unwrap().status,
        loopal_protocol::ThreadGoalStatus::Complete,
    );
}

#[tokio::test]
async fn rejects_when_no_goal_exists() {
    let session = FakeGoalSession::empty();
    let ctx = ctx_with_goal_session(session);
    let result = make_update_goal_tool()
        .execute(json!({"status": "complete"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("no goal exists"));
}

#[tokio::test]
async fn surfaces_disabled_when_no_goal_session() {
    let ctx = ctx_without_goal_session();
    let result = make_update_goal_tool()
        .execute(json!({"status": "complete"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("disabled"));
}
