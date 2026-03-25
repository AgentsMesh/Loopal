use loopal_message::{ContentBlock, MessageRole};
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::PermissionMode;

use super::{make_cancel, make_runner_with_channels};

#[tokio::test]
async fn test_execute_tools_bypass_mode() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Bypass;

    // Create a temp file for Read tool
    let tmp = std::env::temp_dir().join("loopal_exec_tools_test.txt");
    std::fs::write(&tmp, "hello from test").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    let tool_uses = vec![(
        "tc-1".to_string(),
        "Read".to_string(),
        serde_json::json!({"file_path": tmp.to_str().unwrap()}),
    )];

    let completion = runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();
    assert!(
        completion.is_none(),
        "Read tool should not trigger completion"
    );

    // Should have added tool result message
    assert_eq!(runner.params.store.len(), 1);
    let msg = &runner.params.store.messages()[0];
    assert_eq!(msg.role, MessageRole::User);
    assert!(!msg.content.is_empty());

    // Drain events
    let mut found_tool_result = false;
    while let Ok(event) = event_rx.try_recv() {
        if matches!(event.payload, AgentEventPayload::ToolResult { .. }) {
            found_tool_result = true;
        }
    }
    assert!(found_tool_result);

    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn test_execute_tools_supervised_denies_without_approval() {
    // Supervised mode sends Ask → no response from perm channel → Deny
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, perm_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Supervised;

    // Drop perm_tx so the ask returns Deny
    drop(perm_tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tool_uses = vec![(
        "tc-1".to_string(),
        "Write".to_string(),
        serde_json::json!({"file_path": "/tmp/nope.txt", "content": "x"}),
    )];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    // Should have added a denied tool result message
    assert_eq!(runner.params.store.len(), 1);
    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error);
            assert!(content.contains("Permission denied"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[tokio::test]
async fn test_execute_tools_read_allowed_write_denied_in_supervised() {
    // Tests the interleaving: Read (ReadOnly → Allow) and Write (Supervised → Ask → Deny)
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, perm_tx) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::Supervised;

    // Drop perm_tx so the ask returns Deny
    drop(perm_tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    // Create a temp file for Read
    let tmp = std::env::temp_dir().join(format!("loopal_mixed_perm_{}.txt", std::process::id()));
    std::fs::write(&tmp, "mixed test").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

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

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    // Should have 1 message with 2 tool results
    assert_eq!(runner.params.store.len(), 1);
    let msg = &runner.params.store.messages()[0];
    assert_eq!(msg.content.len(), 2);

    // tc-1 (Read) should succeed, tc-2 (Write) should be denied
    match &msg.content[0] {
        ContentBlock::ToolResult { is_error, .. } => {
            assert!(!is_error, "Read should succeed in Supervised mode");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
    match &msg.content[1] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error, "Write should be denied when no approval channel");
            assert!(content.contains("Permission denied"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }

    let _ = std::fs::remove_file(&tmp);
}
