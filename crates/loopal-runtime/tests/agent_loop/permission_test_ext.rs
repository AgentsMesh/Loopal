use std::sync::Arc;

use chrono::Utc;
use loopal_config::Settings;
use loopal_context::{ContextBudget, ContextPipeline, ContextStore};
use loopal_kernel::Kernel;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_runtime::agent_loop::AgentLoopRunner;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, TuiPermissionHandler};
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend};
use loopal_storage::Session;
use loopal_tool_api::{PermissionDecision, PermissionMode};
use tokio::sync::mpsc;

fn make_test_budget() -> ContextBudget {
    ContextBudget {
        context_window: 200_000,
        system_tokens: 0,
        tool_tokens: 0,
        output_reserve: 16_384,
        safety_margin: 10_000,
        message_budget: 173_616,
    }
}

use super::make_runner_with_channels;

#[tokio::test]
async fn test_check_permission_bypass_mode() {
    let (mut runner, _event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Bypass;

    let decision = runner
        .check_permission("id1", "Bash", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_check_permission_supervised_mode_allows_read() {
    let (mut runner, _event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Supervised;

    let decision = runner
        .check_permission("id1", "Read", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_check_permission_ask_mode_approved() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Supervised;

    let perm_tx_clone = perm_tx.clone();
    tokio::spawn(async move {
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
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Supervised;

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
    let (runner, _event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();

    let decision = runner
        .check_permission("id1", "NonExistentTool", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[tokio::test]
async fn test_check_permission_channel_closed_denies() {
    let (event_tx, event_rx) = mpsc::channel(16);
    let (_mbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_ctrl_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (_perm_tx, permission_rx) = mpsc::channel::<bool>(16);

    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        event_tx.clone(),
        mailbox_rx,
        control_rx,
        None,
        Box::new(TuiPermissionHandler::new(event_tx, permission_rx)),
        Box::new(AutoCancelQuestionHandler),
    ));

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

    let tmp_dir =
        std::env::temp_dir().join(format!("loopal_test_perm_closed_{}", std::process::id()));
    let session_manager = SessionManager::with_base_dir(tmp_dir);

    let params = AgentLoopParams {
        kernel,
        session,
        store: ContextStore::new(make_test_budget()),
        model: "claude-sonnet-4-20250514".to_string(),
        compact_model: None,
        system_prompt: "test".to_string(),
        mode: AgentMode::Act,
        permission_mode: PermissionMode::Supervised,
        max_turns: 10,
        frontend,
        session_manager,
        context_pipeline: ContextPipeline::new(),
        tool_filter: None,
        shared: None,
        interactive: true,
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: Default::default(),
        interrupt_tx: std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
        memory_channel: None,
    };

    let runner = AgentLoopRunner::new(params);
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
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, perm_tx) = make_runner_with_channels();
    runner.params.permission_mode = PermissionMode::Supervised;

    // Drop perm_tx so recv returns None
    drop(perm_tx);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let decision = runner
        .check_permission("id1", "Write", &serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(decision, PermissionDecision::Deny);
}
