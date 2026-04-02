//! Integration tests for Plan mode tool filtering and system-reminder wrapping.

use loopal_message::ContentBlock;
use loopal_runtime::AgentMode;
use loopal_runtime::agent_loop::PlanModeState;
use loopal_runtime::plan_file::build_plan_mode_filter;
use loopal_tool_api::PermissionMode;

use super::{make_cancel, make_runner_with_channels};

/// Helper: set up a runner in plan mode with proper PlanModeState.
fn setup_plan_state(runner: &mut loopal_runtime::agent_loop::AgentLoopRunner) {
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: PermissionMode::Supervised,
        tool_filter: build_plan_mode_filter(&runner.params.deps.kernel),
    });
    runner.params.config.mode = AgentMode::Plan;
}

#[tokio::test]
async fn plan_mode_blocks_bash() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    setup_plan_state(&mut runner);

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tool_uses = vec![(
        "tc-1".to_string(),
        "Bash".to_string(),
        serde_json::json!({"command": "echo hello"}),
    )];
    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error);
            assert!(content.contains("Plan mode"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[tokio::test]
async fn plan_mode_allows_read_with_reminder() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    setup_plan_state(&mut runner);
    runner.params.config.permission_mode = PermissionMode::Bypass;

    let tmp = std::env::temp_dir().join(format!("loopal_plan_read_{}.txt", std::process::id()));
    std::fs::write(&tmp, "plan test content").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tool_uses = vec![(
        "tc-1".to_string(),
        "Read".to_string(),
        serde_json::json!({"file_path": tmp.to_str().unwrap()}),
    )];
    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(!is_error, "Read should be allowed in plan mode");
            assert!(content.contains("system-reminder"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }

    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn plan_mode_write_blocks_non_plan_path() {
    let (mut runner, mut event_rx, _mbox_tx, _ctrl_tx, _perm_tx) = make_runner_with_channels();
    setup_plan_state(&mut runner);
    runner.params.config.permission_mode = PermissionMode::Bypass;

    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

    let tool_uses = vec![(
        "tc-1".to_string(),
        "Write".to_string(),
        serde_json::json!({"file_path": "/tmp/not-a-plan.txt", "content": "hack"}),
    )];
    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(is_error);
            assert!(content.contains("only the plan file"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}
