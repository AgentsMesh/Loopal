//! Unit tests for TaskCreate / TaskList / TaskGet / TaskUpdate tools.

use loopal_agent::tools::task::{TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool};
use loopal_test_support::TestFixture;
use loopal_test_support::agent_ctx::agent_tool_context;
use loopal_tool_api::Tool;
use serde_json::json;

#[tokio::test]
async fn task_create_basic() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = TaskCreateTool
        .execute(
            json!({"subject": "Fix bug", "description": "Segfault on startup"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("Fix bug"));
}

#[tokio::test]
async fn task_list_empty() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = TaskListTool.execute(json!({}), &ctx).await.unwrap();
    assert!(!result.is_error);
}

#[tokio::test]
async fn task_list_with_tasks() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    TaskCreateTool
        .execute(json!({"subject": "A", "description": "a"}), &ctx)
        .await
        .unwrap();
    let result = TaskListTool.execute(json!({}), &ctx).await.unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("A"));
}

#[tokio::test]
async fn task_get_existing() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let create = TaskCreateTool
        .execute(json!({"subject": "B", "description": "b"}), &ctx)
        .await
        .unwrap();
    // TaskCreate returns JSON with "id" field in the content string.
    let created: serde_json::Value = serde_json::from_str(&create.content).unwrap();
    let id = created["id"].as_str().unwrap();
    let result = TaskGetTool
        .execute(json!({"taskId": id}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("B"));
}

#[tokio::test]
async fn task_get_nonexistent() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = TaskGetTool
        .execute(json!({"taskId": "999"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
}

#[tokio::test]
async fn task_update_status() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let create = TaskCreateTool
        .execute(json!({"subject": "C", "description": "c"}), &ctx)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_str(&create.content).unwrap();
    let id = created["id"].as_str().unwrap();
    let result = TaskUpdateTool
        .execute(json!({"taskId": id, "status": "completed"}), &ctx)
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("completed"));
}

#[tokio::test]
async fn task_update_nonexistent() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = TaskUpdateTool
        .execute(json!({"taskId": "999", "status": "completed"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_error);
}
