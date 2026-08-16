use loopal_config::Settings;
use loopal_config::{HookConfig, HookEvent};
use loopal_kernel::Kernel;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_runtime::tool_prepare::prepare_tool_action;
use loopal_tool_api::ToolContext;

fn make_kernel_with_hooks(hooks: Vec<HookConfig>) -> Kernel {
    let settings = Settings {
        hooks,
        ..Default::default()
    };
    Kernel::new(settings).expect("Kernel::new with hooks should succeed")
}

fn temp_file(name: &str, content: &str) -> (std::path::PathBuf, ToolContext) {
    let tmp_dir = std::env::temp_dir();
    let path = tmp_dir.join(name);
    std::fs::write(&path, content).unwrap();
    let backend = loopal_backend::LocalBackend::new(
        tmp_dir.clone(),
        None,
        loopal_backend::ResourceLimits::default(),
        "test-session",
    );
    let ctx = ToolContext::new(backend, format!("test-{name}"));
    (path, ctx)
}

async fn run_tool(
    kernel: &Kernel,
    name: &str,
    input: serde_json::Value,
    ctx: &ToolContext,
) -> loopal_error::Result<loopal_tool_api::ToolResult> {
    let action = prepare_tool_action(kernel, "hook-test", name, input)
        .await?
        .into_prepared()?;
    execute_tool(kernel, action, ctx, &loopal_runtime::mode::AgentMode::Act).await
}

#[tokio::test]
async fn test_passing_pre_hook() {
    let kernel = make_kernel_with_hooks(vec![HookConfig {
        event: HookEvent::PreToolUse,
        command: "echo ok".to_string(),
        tool_filter: None,
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }]);
    let (path, ctx) = temp_file("tool_pre_hook_pass.txt", "pre-hook pass content");
    let result = run_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let result = result.expect("passing pre-hook should succeed");
    assert!(!result.is_error);
    assert!(result.content.contains("pre-hook pass content"));
}

#[tokio::test]
async fn test_failing_pre_hook() {
    let kernel = make_kernel_with_hooks(vec![HookConfig {
        event: HookEvent::PreToolUse,
        command: "echo 'denied by hook' >&2; exit 1".to_string(),
        tool_filter: None,
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }]);
    let (path, ctx) = temp_file("tool_pre_hook_fail.txt", "should not read this");
    let result = run_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let error = result.expect_err("failing pre-hook should deny preparation");
    assert!(error.to_string().contains("Pre-hook rejected"));
}

#[tokio::test]
async fn test_post_hook_failure_ignored() {
    let kernel = make_kernel_with_hooks(vec![HookConfig {
        event: HookEvent::PostToolUse,
        command: "exit 1".to_string(),
        tool_filter: None,
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }]);
    let (path, ctx) = temp_file("tool_post_hook_fail.txt", "post hook test content");
    let result = run_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let result = result.expect("post-hook failure should not prevent result");
    assert!(!result.is_error);
    assert!(result.content.contains("post hook test content"));
}

#[tokio::test]
async fn test_both_pre_and_post_hooks() {
    let kernel = make_kernel_with_hooks(vec![
        HookConfig {
            event: HookEvent::PreToolUse,
            command: "echo pre-hook-ok".to_string(),
            tool_filter: None,
            timeout_ms: 5000,
            hook_type: Default::default(),
            url: None,
            headers: Default::default(),
            prompt: None,
            model: None,
            condition: None,
            id: None,
        },
        HookConfig {
            event: HookEvent::PostToolUse,
            command: "echo post-hook-ok".to_string(),
            tool_filter: None,
            timeout_ms: 5000,
            hook_type: Default::default(),
            url: None,
            headers: Default::default(),
            prompt: None,
            model: None,
            condition: None,
            id: None,
        },
    ]);
    let (path, ctx) = temp_file("tool_both_hooks.txt", "both hooks content");
    let result = run_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let result = result.expect("both hooks passing should allow execution");
    assert!(!result.is_error);
    assert!(result.content.contains("both hooks content"));
}
