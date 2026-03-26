//! Tests for AttemptCompletion detection in execute_tools().

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_message::ContentBlock;
use loopal_tool_api::{COMPLETION_PREFIX, Tool, ToolContext, ToolResult};
use loopal_tool_api::{PermissionLevel, PermissionMode};

use super::{make_cancel, make_runner_with_channels};

/// Fake tool that returns "{COMPLETION_PREFIX}{input.result}".
struct FakeCompletionTool;

#[async_trait]
impl Tool for FakeCompletionTool {
    fn name(&self) -> &str {
        "AttemptCompletion"
    }
    fn description(&self) -> &str {
        "test completion"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    async fn execute(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let result = input
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("done");
        Ok(ToolResult::success(format!("{COMPLETION_PREFIX}{result}")))
    }
}

#[tokio::test]
async fn test_execute_tools_detects_attempt_completion() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;

    // Register fake completion tool
    let kernel = std::sync::Arc::get_mut(&mut runner.params.deps.kernel).unwrap();
    kernel.register_tool(Box::new(FakeCompletionTool));

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tool_uses = vec![(
        "tc-1".to_string(),
        "AttemptCompletion".to_string(),
        serde_json::json!({"result": "task finished successfully"}),
    )];

    let completion = runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();
    assert_eq!(completion, Some("task finished successfully".to_string()));
}

#[tokio::test]
async fn test_execute_tools_completion_mixed_with_normal_tool() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;

    let kernel = std::sync::Arc::get_mut(&mut runner.params.deps.kernel).unwrap();
    kernel.register_tool(Box::new(FakeCompletionTool));

    // Create temp file for Read tool
    let tmp = std::env::temp_dir().join(format!("la_comp_mixed_{}.txt", std::process::id()));
    std::fs::write(&tmp, "data").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tool_uses = vec![
        (
            "tc-1".to_string(),
            "Read".to_string(),
            serde_json::json!({"file_path": tmp.to_str().unwrap()}),
        ),
        (
            "tc-2".to_string(),
            "AttemptCompletion".to_string(),
            serde_json::json!({"result": "all done"}),
        ),
    ];

    let completion = runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();
    assert_eq!(completion, Some("all done".to_string()));

    // Both tool results should be in messages
    assert_eq!(runner.params.store.len(), 1);
    assert_eq!(runner.params.store.messages()[0].content.len(), 2);

    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_execute_tools_error_result_not_detected_as_completion() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    // Nonexistent tool → error result containing the prefix should NOT match
    // because is_error = true
    let tool_uses = vec![(
        "tc-1".to_string(),
        "NonExistentTool".to_string(),
        serde_json::json!({}),
    )];

    let completion = runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();
    assert!(
        completion.is_none(),
        "error results should not trigger completion"
    );

    // Verify tool result is error
    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        ContentBlock::ToolResult { is_error, .. } => assert!(is_error),
        other => panic!("expected ToolResult, got {other:?}"),
    }
}
