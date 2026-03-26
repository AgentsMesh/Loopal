use loopal_config::Settings;
use loopal_kernel::Kernel;
use loopal_runtime::tool_pipeline::execute_tool;
use loopal_tool_api::ToolContext;
use std::path::PathBuf;

fn make_kernel() -> Kernel {
    Kernel::new(Settings::default()).expect("Kernel::new should succeed")
}

fn make_ctx() -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        PathBuf::from("/tmp"),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        backend,
        session_id: "test-session".to_string(),
        shared: None,
        memory_channel: None,
        output_tail: None,
    }
}

fn make_ctx_for(cwd: PathBuf) -> ToolContext {
    let backend =
        loopal_backend::LocalBackend::new(cwd, None, loopal_backend::ResourceLimits::default());
    ToolContext {
        backend,
        session_id: "test-session".to_string(),
        shared: None,
        memory_channel: None,
        output_tail: None,
    }
}

#[tokio::test]
async fn test_execute_tool_not_found() {
    let kernel = make_kernel();
    let ctx = make_ctx();
    let result = execute_tool(
        &kernel,
        "NonExistentTool",
        serde_json::json!({}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;
    assert!(result.is_err(), "executing a nonexistent tool should fail");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("NonExistentTool"),
        "error should mention the tool name, got: {err_msg}",
    );
}

#[tokio::test]
async fn test_execute_read_tool_on_temp_file() {
    let kernel = make_kernel();
    let tmp_dir = std::env::temp_dir();
    let test_file = tmp_dir.join("loopal_test_tool_pipeline_read.txt");
    std::fs::write(&test_file, "hello from test").unwrap();

    let ctx = make_ctx_for(tmp_dir.clone());
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": test_file.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;

    let _ = std::fs::remove_file(&test_file);
    let result = result.expect("Read tool should succeed");
    assert!(!result.is_error, "Read tool should not report error");
    assert!(result.content.contains("hello from test"));
}

#[tokio::test]
async fn test_execute_read_tool_missing_file() {
    let kernel = make_kernel();
    let ctx = make_ctx();
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": "/tmp/nonexistent_file_loopal_test_xyz_12345.txt"}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;

    if let Ok(r) = result {
        assert!(r.is_error, "reading missing file should set is_error=true")
    }
}

#[tokio::test]
async fn test_execute_tool_in_plan_mode() {
    let kernel = make_kernel();
    let tmp_dir = std::env::temp_dir();
    let test_file = tmp_dir.join("loopal_test_tool_pipeline_plan.txt");
    std::fs::write(&test_file, "plan mode test content").unwrap();

    let ctx = make_ctx_for(tmp_dir.clone());
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": test_file.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Plan,
    )
    .await;

    let _ = std::fs::remove_file(&test_file);
    let result = result.expect("Read tool should succeed even in plan mode");
    assert!(!result.is_error);
}

#[tokio::test]
async fn test_large_tool_output_is_truncated_and_saved() {
    let kernel = make_kernel();
    let tmp_dir = std::env::temp_dir();
    let test_file = tmp_dir.join("loopal_test_tool_pipeline_large.txt");

    let long_line = "X".repeat(5000);
    let content: String = (0..100)
        .map(|_| long_line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&test_file, &content).unwrap();

    let ctx = make_ctx_for(tmp_dir.clone());
    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": test_file.to_str().unwrap()}),
        &ctx,
        &loopal_runtime::mode::AgentMode::Act,
    )
    .await;

    let _ = std::fs::remove_file(&test_file);
    let result = result.expect("Read tool should succeed");
    assert!(!result.is_error);
    assert!(
        result.content.len() <= 500_000,
        "output should be truncated"
    );
    assert!(
        result.content.contains("truncated"),
        "should contain truncation notice"
    );
    assert!(
        result.content.contains("[Full output saved to:"),
        "should contain saved file path"
    );

    let saved_marker = "[Full output saved to: ";
    let start = result.content.find(saved_marker).unwrap() + saved_marker.len();
    let end = result.content[start..].find(']').unwrap() + start;
    let saved_path = &result.content[start..end];
    let saved = std::fs::read_to_string(saved_path).expect("saved file should exist");
    assert!(
        saved.len() > 400_000,
        "saved file should contain full output"
    );
    let _ = std::fs::remove_file(saved_path);
}
