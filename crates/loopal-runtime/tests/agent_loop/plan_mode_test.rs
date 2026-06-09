use loopal_provider_api::ContentBlock;
use loopal_runtime::AgentMode;
use loopal_runtime::agent_loop::LifecycleMode;
use loopal_tool_api::PermissionMode;

use super::{in_turn, make_runner, make_runner_with_channels, make_turn_ctx};

#[tokio::test]
async fn enter_plan_mode_denied_by_default_frontend() {
    let (mut runner, _rx) = make_runner();
    runner.params.config.permission_mode = PermissionMode::Bypass;

    let tool_uses = vec![(
        "tc-1".to_string(),
        "EnterPlanMode".to_string(),
        serde_json::json!({}),
    )];
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tool_uses,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    // DenyAllHandler returns Deny for request_permission → mode stays Act.
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
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tool_uses,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let msg = &runner.turns.view().messages()[0];
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
    runner.params.config.lifecycle = LifecycleMode::Ephemeral;

    let tool_uses = vec![(
        "tc-1".to_string(),
        "EnterPlanMode".to_string(),
        serde_json::json!({}),
    )];
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tool_uses,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let msg = &runner.turns.view().messages()[0];
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
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tool_uses,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let msg = &runner.turns.view().messages()[0];
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
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tool_uses,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let msg = &runner.turns.view().messages()[0];
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
async fn permission_switch_during_plan_updates_snapshot() {
    use loopal_protocol::ControlCommand;
    use loopal_runtime::agent_loop::PlanModeState;

    let (mut runner, _rx, _mbox, ctrl_tx, _int) = make_runner_with_channels();
    runner.params.config.mode = AgentMode::Plan;
    runner.params.config.permission_mode = PermissionMode::Bypass;
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: PermissionMode::Bypass,
        tool_filter: Default::default(),
    });

    ctrl_tx
        .send(ControlCommand::PermissionModeSwitch("ask_any_write".into()))
        .await
        .unwrap();

    // wait_for_input processes the control then blocks on the next select;
    // bound the block so the test returns once the control is handled.
    tokio::select! {
        _ = runner.wait_for_input() => {}
        _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {}
    }

    assert_eq!(
        runner.params.config.permission_mode,
        PermissionMode::AskAnyWrite,
        "runtime permission_mode must reflect the switch"
    );
    assert_eq!(
        runner
            .params
            .config
            .plan_state
            .as_ref()
            .unwrap()
            .previous_permission_mode,
        PermissionMode::AskAnyWrite,
        "plan snapshot must be updated so plan exit does not revert the switch"
    );
}

#[tokio::test]
async fn exit_plan_restores_snapshot_permission_and_emits_event() {
    use loopal_protocol::AgentEventPayload;
    use loopal_runtime::agent_loop::PlanModeState;

    let (mut runner, mut rx) = make_runner();
    // Snapshot carries ask_any_write (the post-mid-plan-switch state); the
    // live value is deliberately wrong to prove restore is snapshot-driven.
    runner.params.config.plan_state = Some(PlanModeState {
        previous_mode: AgentMode::Act,
        previous_permission_mode: PermissionMode::AskAnyWrite,
        tool_filter: Default::default(),
    });
    runner.params.config.mode = AgentMode::Plan;
    runner.params.config.permission_mode = PermissionMode::Bypass;

    let plan_dir = runner.plan_file.path().parent().unwrap();
    std::fs::create_dir_all(plan_dir).unwrap();
    std::fs::write(runner.plan_file.path(), "# Plan\nStep 1").unwrap();

    let tool_uses = vec![(
        "tc-1".to_string(),
        "ExitPlanMode".to_string(),
        serde_json::json!({}),
    )];
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tool_uses,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!(
        runner.params.config.permission_mode,
        PermissionMode::AskAnyWrite,
        "restore must honor the snapshot, not keep the stale Bypass"
    );

    let mut saw_permission_changed = false;
    while let Ok(ev) = rx.try_recv() {
        if let AgentEventPayload::PermissionModeChanged { mode } = ev.payload {
            assert_eq!(mode, "ask_any_write");
            saw_permission_changed = true;
        }
    }
    assert!(
        saw_permission_changed,
        "plan exit must emit PermissionModeChanged so the observable stays in sync"
    );
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
    let mut turn_ctx = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn_ctx,
        tool_uses,
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!(runner.params.config.mode, AgentMode::Act);
    assert!(runner.params.config.plan_state.is_none());

    let msg = &runner.turns.view().messages()[0];
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
