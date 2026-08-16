use std::sync::Arc;

use loopal_config::{
    HookConfig, HookEvent, NetworkPolicy, ResolvedPolicy, SandboxPolicy, Settings,
};
use loopal_kernel::Kernel;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_tool_api::PermissionMode;
use serde_json::json;

use super::{in_turn, make_runner_with_kernel, make_turn_ctx};

#[tokio::test]
async fn sandbox_permission_is_derived_from_rewritten_path() {
    let cwd = std::env::temp_dir().join(format!("loopal-rewrite-sandbox-{}", std::process::id()));
    std::fs::create_dir_all(&cwd).unwrap();
    let rewritten = "/var/loopal-rewritten-sandbox.txt";
    let output = json!({
        "updated_input": {"file_path": rewritten, "content": "rewritten"}
    });
    let settings = Settings {
        hooks: vec![HookConfig {
            event: HookEvent::PreToolUse,
            command: format!("printf '%s' '{output}'"),
            tool_filter: Some(vec!["Write".into()]),
            timeout_ms: 5_000,
            hook_type: Default::default(),
            url: None,
            headers: Default::default(),
            prompt: None,
            model: None,
            condition: None,
            id: None,
        }],
        ..Default::default()
    };
    let (mut runner, mut events, permission_tx) =
        make_runner_with_kernel(Arc::new(Kernel::new(settings).unwrap()));
    runner.params.config.permission_mode = PermissionMode::AskAnyWrite;
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        cwd.clone(),
        Some(ResolvedPolicy {
            policy: SandboxPolicy::DefaultWrite,
            writable_paths: vec![cwd.clone()],
            deny_write_globs: vec![],
            deny_read_globs: vec![],
            network: NetworkPolicy::default(),
        }),
        loopal_backend::ResourceLimits::default(),
        "rewrite-sandbox",
    );
    let (input_tx, mut input_rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if let AgentEventPayload::ToolPermissionRequest { input, .. } = event.payload {
                let _ = input_tx.send(input).await;
            }
        }
    });
    let original = cwd.join("original.txt");
    let mut turn = make_turn_ctx();
    let execution = in_turn(runner.execute_tools(
        &mut turn,
        vec![(
            "write-sandbox".into(),
            "Write".into(),
            json!({"file_path": original, "content": "original"}),
        )],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ));
    tokio::pin!(execution);
    let input = tokio::select! {
        input = input_rx.recv() => input.unwrap(),
        result = &mut execution => panic!("execution ended before approval: {result:?}"),
    };
    assert_eq!(input["file_path"], rewritten);
    let reason = input["sandbox_approval_reason"].as_str().unwrap();
    assert!(reason.contains("loopal-rewritten-sandbox.txt"));
    assert!(!reason.contains("original.txt"));
    permission_tx.send(false).await.unwrap();
    execution.await.unwrap();
    let _ = std::fs::remove_dir_all(cwd);
}

async fn rewritten_denial(
    tool: &str,
    original: serde_json::Value,
    updated: serde_json::Value,
) -> String {
    let output = json!({"updated_input": updated});
    let settings = Settings {
        hooks: vec![HookConfig {
            event: HookEvent::PreToolUse,
            command: format!("printf '%s' '{output}'"),
            tool_filter: Some(vec![tool.into()]),
            timeout_ms: 5_000,
            hook_type: Default::default(),
            url: None,
            headers: Default::default(),
            prompt: None,
            model: None,
            condition: None,
            id: None,
        }],
        ..Default::default()
    };
    let (mut runner, mut events, _) =
        make_runner_with_kernel(Arc::new(Kernel::new(settings).unwrap()));
    runner.params.config.permission_mode = PermissionMode::Bypass;
    tokio::spawn(async move { while events.recv().await.is_some() {} });
    let mut turn = make_turn_ctx();
    in_turn(runner.execute_tools(
        &mut turn,
        vec![("rewritten-denial".into(), tool.into(), original)],
        loopal_runtime::agent_loop::StreamingToolHandle::empty(),
    ))
    .await
    .unwrap();
    match &runner.turns.view().messages()[0].content[0] {
        ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert!(*is_error);
            content.clone()
        }
        block => panic!("expected tool result, got {block:?}"),
    }
}

#[tokio::test]
async fn rewritten_wire_ref_and_tool_precheck_are_rejected_before_permission() {
    let wire = rewritten_denial(
        "Write",
        json!({"file_path": "/tmp/safe", "content": "safe"}),
        json!({"file_path": "/tmp/safe", "content": "<secret_ref:token>"}),
    )
    .await;
    assert!(wire.contains("Pre-hook produced invalid tool input"));

    let precheck = rewritten_denial(
        "Bash",
        json!({"command": "printf safe"}),
        json!({"command": "rm -rf /"}),
    )
    .await;
    assert!(precheck.contains("Sandbox:"));
}
