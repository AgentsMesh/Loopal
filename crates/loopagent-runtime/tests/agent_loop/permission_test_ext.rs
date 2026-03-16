use std::sync::Arc;

use chrono::Utc;
use loopagent_context::ContextPipeline;
use loopagent_kernel::Kernel;
use loopagent_runtime::agent_loop::AgentLoopRunner;
use loopagent_runtime::{AgentLoopParams, AgentMode, SessionManager};
use loopagent_storage::Session;
use loopagent_types::config::Settings;
use loopagent_types::permission::{PermissionDecision, PermissionMode};
use tokio::sync::mpsc;

use super::make_runner_with_channels;

#[tokio::test]
async fn test_check_permission_bypass_mode() {
    let (mut runner, _event_rx, _input_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::BypassPermissions;

    let decision = runner
        .check_permission("id1", "Bash", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_check_permission_plan_mode_denies_write() {
    let (mut runner, _event_rx, _input_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Plan;

    let decision = runner
        .check_permission("id1", "Write", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Deny);
}

#[tokio::test]
async fn test_check_permission_plan_mode_allows_read() {
    let (mut runner, _event_rx, _input_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Plan;

    let decision = runner
        .check_permission("id1", "Read", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_check_permission_ask_mode_approved() {
    let (mut runner, mut event_rx, _input_tx, perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Default;

    // Spawn approval in background
    let perm_tx_clone = perm_tx.clone();
    tokio::spawn(async move {
        // Wait for the permission request event
        let _event = event_rx.recv().await;
        perm_tx_clone.send(true).await.unwrap();
    });

    let decision = runner
        .check_permission("id1", "Write", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_check_permission_ask_mode_denied() {
    let (mut runner, mut event_rx, _input_tx, perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Default;

    let perm_tx_clone = perm_tx.clone();
    tokio::spawn(async move {
        let _event = event_rx.recv().await;
        perm_tx_clone.send(false).await.unwrap();
    });

    let decision = runner
        .check_permission("id1", "Write", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Deny);
}

#[tokio::test]
async fn test_check_permission_unknown_tool_allows() {
    let (mut runner, _event_rx, _input_tx, _perm_tx) = make_runner_with_channels();

    let decision = runner
        .check_permission("id1", "NonExistentTool", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_check_permission_channel_closed_denies() {
    // send_ok is false when event_tx is closed
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_input_tx, input_rx) = mpsc::channel(16);
    let (_perm_tx, permission_rx) = mpsc::channel(16);

    let kernel = Arc::new(Kernel::new(Settings::default()).unwrap());
    let session = Session {
        id: "test-perm-closed".to_string(),
        title: "".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        cwd: "/tmp".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        mode: "default".to_string(),
    };

    let tmp_dir = std::env::temp_dir().join(format!(
        "loopagent_test_perm_closed_{}",
        std::process::id()
    ));
    let session_manager = SessionManager::with_base_dir(tmp_dir);

    let params = AgentLoopParams {
        kernel,
        session,
        messages: Vec::new(),
        model: "claude-sonnet-4-20250514".to_string(),
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Default, // Will Ask for Write
        max_turns: 10,
        event_tx,
        input_rx,
        permission_rx,
        session_manager,
        context_pipeline: ContextPipeline::new(),
    };

    let mut runner = AgentLoopRunner::new(params);
    // Close event_rx so send fails
    drop(event_rx);

    let decision = runner
        .check_permission("id1", "Write", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Deny);
}

#[tokio::test]
async fn test_check_permission_rx_closed_denies() {
    // permission_rx.recv() returns None
    let (mut runner, mut event_rx, _input_tx, perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Default;

    // Drop perm_tx so recv returns None
    drop(perm_tx);

    // Need to drain the ToolPermissionRequest event
    tokio::spawn(async move {
        while event_rx.recv().await.is_some() {}
    });

    let decision = runner
        .check_permission("id1", "Write", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Deny);
}
