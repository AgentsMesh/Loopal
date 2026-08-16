use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_runtime::AgentMode;
use loopal_runtime::agent_loop::StreamingToolHandle;
use loopal_tool_api::PermissionMode;

use super::{in_turn, make_runner_with_channels, make_turn_ctx};

#[tokio::test]
async fn enter_plan_success_sets_snapshot_emits_mode_and_creates_directory() {
    let (mut runner, mut events, _mailbox, _control, permission) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    permission.send(true).await.unwrap();
    let mut turn = make_turn_ctx();

    in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "enter".into(),
            "EnterPlanMode".into(),
            serde_json::json!({}),
        )],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!(runner.params.config.mode, AgentMode::Plan);
    let state = runner.params.config.plan_state.as_ref().unwrap();
    assert_eq!(state.previous_mode, AgentMode::Act);
    assert_eq!(state.previous_permission_mode, PermissionMode::AskAnyWrite);
    assert!(runner.plan_file.path().parent().unwrap().is_dir());
    let block = &runner.turns.view().messages()[0].content[0];
    assert!(matches!(
        block,
        ContentBlock::ToolResult { content, is_error: false, .. }
            if content.contains("Entered plan mode") && content.contains("No plan file yet")
    ));
    assert!(std::iter::from_fn(|| events.try_recv().ok()).any(|event| {
        matches!(
            event.payload,
            AgentEventPayload::ModeChanged { ref mode } if mode == "plan"
        )
    }));
}

#[tokio::test]
async fn enter_plan_reports_existing_plan_file() {
    let (mut runner, _events, _mailbox, _control, permission) = make_runner_with_channels();
    std::fs::create_dir_all(runner.plan_file.path().parent().unwrap()).unwrap();
    std::fs::write(runner.plan_file.path(), "# Existing").unwrap();
    permission.send(true).await.unwrap();
    let mut turn = make_turn_ctx();

    in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "enter".into(),
            "EnterPlanMode".into(),
            serde_json::json!({}),
        )],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    let block = &runner.turns.view().messages()[0].content[0];
    assert!(matches!(
        block,
        ContentBlock::ToolResult { content, is_error: false, .. }
            if content.contains("already exists")
    ));
}

#[tokio::test]
async fn enter_plan_directory_failure_rolls_back_mode_and_snapshot() {
    let (mut runner, mut events, _mailbox, _control, permission) = make_runner_with_channels();
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    std::fs::create_dir_all(&runner.params.session.cwd).unwrap();
    let loopal_path = std::path::Path::new(&runner.params.session.cwd).join(".loopal");
    std::fs::write(&loopal_path, "blocks directory creation").unwrap();
    permission.send(true).await.unwrap();
    let mut turn = make_turn_ctx();

    in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "enter".into(),
            "EnterPlanMode".into(),
            serde_json::json!({}),
        )],
        StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();

    assert_eq!(runner.params.config.mode, AgentMode::Act);
    assert_eq!(
        runner.params.config.permission_mode,
        PermissionMode::AskAnyWrite
    );
    assert!(runner.params.config.plan_state.is_none());
    let block = &runner.turns.view().messages()[0].content[0];
    assert!(matches!(
        block,
        ContentBlock::ToolResult { content, is_error: true, .. }
            if content.contains("Plan mode was not entered")
    ));
    let modes = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event.payload {
            AgentEventPayload::ModeChanged { mode } => Some(mode),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(modes, ["plan", "act"]);
}
