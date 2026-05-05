use loopal_config::Settings;
use loopal_config::{HookConfig, HookEvent};
use loopal_kernel::Kernel;
use loopal_runtime::tool_pipeline::execute_tool;
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
    );
    let ctx = ToolContext::new(backend, format!("test-{name}"));
    (path, ctx)
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
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
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
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let result = result.expect("failing pre-hook should return Ok(error)");
    assert!(result.is_error);
    assert!(result.content.contains("Pre-hook rejected"));
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
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let result = result.expect("post-hook failure should not prevent result");
    assert!(!result.is_error);
    assert!(result.content.contains("post hook test content"));
}

#[tokio::test]
async fn test_filtered_pre_hook_not_matching() {
    let kernel = make_kernel_with_hooks(vec![HookConfig {
        event: HookEvent::PreToolUse,
        command: "exit 1".to_string(),
        tool_filter: Some(vec!["Bash".to_string()]),
        timeout_ms: 5000,
        hook_type: Default::default(),
        url: None,
        headers: Default::default(),
        prompt: None,
        model: None,
        condition: None,
        id: None,
    }]);
    let (path, ctx) = temp_file("tool_filtered_hook.txt", "filtered hook content");
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let result = result.expect("filtered hook should not block unmatched tool");
    assert!(!result.is_error);
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
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": path.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    let result = result.expect("both hooks passing should allow execution");
    assert!(!result.is_error);
    assert!(result.content.contains("both hooks content"));
}
