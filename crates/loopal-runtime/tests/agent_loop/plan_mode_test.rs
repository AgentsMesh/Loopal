//! Integration tests for Plan mode EnterPlanMode / ExitPlanMode interception.

use loopal_message::ContentBlock;
use loopal_runtime::AgentMode;
use loopal_runtime::agent_loop::LifecycleMode;
use loopal_tool_api::PermissionMode;

use super::{make_cancel, make_runner};

#[tokio::test]
async fn enter_plan_mode_denied_by_default_frontend() {
    let (mut runner, _rx) = make_runner();
    runner.params.config.permission_mode = PermissionMode::Bypass;

    let tool_uses = vec![(
        "tc-1".to_string(),
        "EnterPlanMode".to_string(),
        serde_json::json!({}),
    )];
    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    // AutoDenyHandler returns Deny for request_permission → mode stays Act.
    assert_eq!(runner.params.config.mode, AgentMode::Act);
}

#[tokio::test]
async fn enter_plan_when_already_in_plan_returns_error() {
    let (mut runner, _rx) = make_runner();
    runner.params.config.mode = AgentMode::Plan;

    let tool_uses = vec![(
        "tc-1".to_string(),
        "EnterPlanMode".to_string(),
        serde_json::json!({}),
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
            assert!(content.contains("Already in plan mode"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[tokio::test]
async fn enter_plan_blocked_for_task_lifecycle() {
    let (mut runner, _rx) = make_runner();
    runner.params.config.lifecycle = LifecycleMode::Task;

    let tool_uses = vec![(
        "tc-1".to_string(),
        "EnterPlanMode".to_string(),
        serde_json::json!({}),
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
            assert!(content.contains("agent context"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[tokio::test]
async fn exit_plan_when_not_in_plan_returns_error() {
    let (mut runner, _rx) = make_runner();
    assert_eq!(runner.params.config.mode, AgentMode::Act);

    let tool_uses = vec![(
        "tc-1".to_string(),
        "ExitPlanMode".to_string(),
        serde_json::json!({}),
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
            assert!(content.contains("not in plan mode"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[tokio::test]
async fn exit_plan_without_plan_file_returns_error() {
    let (mut runner, _rx) = make_runner();
    runner.params.config.mode = AgentMode::Plan;

    let tool_uses = vec![(
        "tc-1".to_string(),
        "ExitPlanMode".to_string(),
        serde_json::json!({}),
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
            assert!(content.contains("No plan file"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[tokio::test]
async fn exit_plan_with_plan_file_approves_and_restores_mode() {
    let (mut runner, _rx) = make_runner();

    use loopal_runtime::agent_loop::PlanModeState;
    use loopal_runtime::plan_file::build_plan_mode_filter;
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: PermissionMode::Bypass,
        tool_filter: build_plan_mode_filter(&runner.params.deps.kernel),
    });
    runner.params.config.mode = AgentMode::Plan;

    let plan_dir = runner.plan_file.path().parent().unwrap();
    std::fs::create_dir_all(plan_dir).unwrap();
    std::fs::write(runner.plan_file.path(), "# Test Plan\nStep 1").unwrap();

    let tool_uses = vec![(
        "tc-1".to_string(),
        "ExitPlanMode".to_string(),
        serde_json::json!({}),
    )];
    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    assert_eq!(runner.params.config.mode, AgentMode::Act);
    assert!(runner.params.config.plan_state.is_none());

    let msg = &runner.params.store.messages()[0];
    match &msg.content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(!is_error);
            assert!(content.contains("approved"));
            assert!(content.contains("Test Plan"));
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}
