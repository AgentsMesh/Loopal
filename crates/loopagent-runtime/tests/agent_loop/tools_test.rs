use loopagent_types::event::AgentEvent;
use loopagent_types::message::{ContentBlock, MessageRole};
use loopagent_types::permission::PermissionMode;

use super::make_runner_with_channels;

#[tokio::test]
async fn test_execute_tools_bypass_mode() {
    let (mut runner, mut event_rx, _input_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::BypassPermissions;

    // Create a temp file for Read tool
    let tmp = std::env::temp_dir().join("loopagent_exec_tools_test.txt");
    std::fs::write(&tmp, "hello from test").unwrap();
    runner.tool_ctx.cwd = std::env::temp_dir();

    let tool_uses = vec![(
        "tc-1".to_string(),
        "Read".to_string(),
        serde_json::json!({"file_path": tmp.to_str().unwrap()}),
    )];

    runner.execute_tools(tool_uses).await.unwrap();

    // Should have added tool result message
    assert_eq!(runner.params.messages.len(), 1);
    let msg = &runner.params.messages[0];
    assert_eq!(msg.role, MessageRole::User);
    assert!(!msg.content.is_empty());

    // Drain events
    let mut found_tool_result = false;
    while let Ok(event) = event_rx.try_recv() {
        if matches!(event, AgentEvent::ToolResult { .. }) {
            found_tool_result = true;
        }
    }
    assert!(found_tool_result);

    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_execute_tools_denied_in_plan_mode() {
    let (mut runner, _event_rx, _input_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Plan;

    let tool_uses = vec![(
        "tc-1".to_string(),
        "Write".to_string(),
        serde_json::json!({"file_path": "/tmp/nope.txt", "content": "x"}),
    )];

    runner.execute_tools(tool_uses).await.unwrap();

    // Should have added a denied tool result message
    assert_eq!(runner.params.messages.len(), 1);
    let msg = &runner.params.messages[0];
    match &msg.content[0] {
        ContentBlock::ToolResult { content, is_error, .. } => {
            assert!(is_error);
            assert!(content.contains("Permission denied"));
        }
        other => panic!("expected ToolResult, got {:?}", other),
    }
}

#[tokio::test]
async fn test_execute_tools_multiple_with_deny_and_allow() {
    // Tests the interleaving of denied and approved tools in execute_tools
    let (mut runner, _event_rx, _input_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Plan; // Only allows ReadOnly

    // Create a temp file for Read
    let tmp = std::env::temp_dir().join(format!("loopagent_mixed_perm_{}.txt", std::process::id()));
    std::fs::write(&tmp, "mixed test").unwrap();
    runner.tool_ctx.cwd = std::env::temp_dir();

    let tool_uses = vec![
        (
            "tc-1".to_string(),
            "Read".to_string(),
            serde_json::json!({"file_path": tmp.to_str().unwrap()}),
        ),
        (
            "tc-2".to_string(),
            "Write".to_string(),
            serde_json::json!({"file_path": "/tmp/nope.txt", "content": "x"}),
        ),
    ];

    runner.execute_tools(tool_uses).await.unwrap();

    // Should have 1 message with 2 tool results
    assert_eq!(runner.params.messages.len(), 1);
    let msg = &runner.params.messages[0];
    assert_eq!(msg.content.len(), 2);

    // Results should be ordered by original index
    // tc-1 (Read) should succeed, tc-2 (Write) should be denied
    match &msg.content[0] {
        ContentBlock::ToolResult { is_error, .. } => {
            assert!(!is_error, "Read should succeed in Plan mode");
        }
        other => panic!("expected ToolResult, got {:?}", other),
    }
    match &msg.content[1] {
        ContentBlock::ToolResult { content, is_error, .. } => {
            assert!(*is_error, "Write should be denied in Plan mode");
            assert!(content.contains("Permission denied"));
        }
        other => panic!("expected ToolResult, got {:?}", other),
    }

    let _ = std::fs::remove_file(&tmp);
}
