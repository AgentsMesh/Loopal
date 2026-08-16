use std::sync::Arc;

use loopal_config::{HookConfig, HookEvent, Settings};
use loopal_kernel::Kernel;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_tool_api::PermissionMode;
use serde_json::json;

use super::{in_turn, make_runner_with_kernel, make_turn_ctx};

fn rewrite_hook(updated_input: &str) -> HookConfig {
    HookConfig {
        event: HookEvent::PreToolUse,
        command: format!("printf '%s' '{updated_input}'"),
        tool_filter: None,
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }
}

fn runner_with_hook(
    updated_input: &str,
) -> (
    loopal_runtime::agent_loop::AgentLoopRunner,
    tokio::sync::mpsc::Receiver<loopal_protocol::AgentEvent>,
    tokio::sync::mpsc::Sender<bool>,
) {
    let settings = Settings {
        hooks: vec![rewrite_hook(updated_input)],
        ..Default::default()
    };
    make_runner_with_kernel(Arc::new(Kernel::new(settings).unwrap()))
}

#[tokio::test]
async fn rewritten_action_is_the_action_permission_classifies() {
    let target = std::env::temp_dir().join(format!("loopal-rewritten-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&target);
    let hook_target = target.to_string_lossy().replace('\\', "/");
    let rewrite = json!({
        "updated_input": {"file_path": hook_target.clone(), "content": "rewritten"}
    });
    let (mut runner, mut events, permission_tx) = runner_with_hook(&rewrite.to_string());
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    let (input_tx, mut input_rx) = tokio::sync::mpsc::channel(1);
    let event_task = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if let AgentEventPayload::ToolPermissionRequest { input, .. } = event.payload {
                input_tx.send(input).await.unwrap();
                break;
            }
        }
    });
    let mut turn = make_turn_ctx();
    let execution = in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "write-1".into(),
            "Write".into(),
            json!({"file_path": "/tmp/original-never-written", "content": "original"}),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ));
    tokio::pin!(execution);
    let approved_input = tokio::select! {
        input = input_rx.recv() => input.unwrap(),
        result = &mut execution => panic!("execution ended before approval: {result:?}"),
    };
    assert_eq!(approved_input["file_path"], json!(hook_target));
    permission_tx.send(true).await.unwrap();
    let _ = execution.await.unwrap();
    event_task.await.unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "rewritten");
    let _ = std::fs::remove_file(target);
}

#[tokio::test]
async fn invalid_rewrite_is_denied_before_execution() {
    let (mut runner, mut events, _permission_tx) = runner_with_hook(r#"{"updated_input":{}}"#);
    runner.params.config.permission_mode = PermissionMode::Bypass;
    tokio::spawn(async move { while events.recv().await.is_some() {} });
    let mut turn = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "write-1".into(),
            "Write".into(),
            json!({"file_path": "/tmp/never-written", "content": "original"}),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();
    match &runner.turns.view().messages()[0].content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(*is_error);
            assert!(content.contains("Pre-hook produced invalid tool input"));
        }
        block => panic!("expected tool result, got {block:?}"),
    }
}

#[tokio::test]
async fn reserved_sandbox_annotation_collision_fails_closed() {
    let (mut runner, mut events) = super::make_runner();
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    tokio::spawn(async move { while events.recv().await.is_some() {} });
    let mut turn = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "write-1".into(),
            "Write".into(),
            json!({
                "file_path": "/var/loopal-collision-test",
                "content": "x",
                "sandbox_approval_reason": "model supplied"
            }),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();
    match &runner.turns.view().messages()[0].content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(*is_error);
            assert!(content.contains("reserved sandbox permission annotation"));
        }
        block => panic!("expected tool result, got {block:?}"),
    }
}
