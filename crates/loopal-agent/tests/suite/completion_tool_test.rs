//! Unit tests for AttemptCompletionTool.

use loopal_agent::tools::completion::AttemptCompletionTool;
use loopal_test_support::TestFixture;
use loopal_test_support::agent_ctx::agent_tool_context;
use loopal_tool_api::Tool;
use serde_json::json;

#[tokio::test]
async fn completion_with_result() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = AttemptCompletionTool
        .execute(json!({"result": "All done"}), &ctx)
        .await
        .unwrap();
    assert!(result.is_completion);
    assert_eq!(result.content, "All done");
}

#[tokio::test]
async fn completion_default_message() {
    let fixture = TestFixture::new();
    let (ctx, _) = agent_tool_context(&fixture);
    let result = AttemptCompletionTool
        .execute(json!({}), &ctx)
        .await
        .unwrap();
    assert!(result.is_completion);
    assert!(!result.content.is_empty());
}
