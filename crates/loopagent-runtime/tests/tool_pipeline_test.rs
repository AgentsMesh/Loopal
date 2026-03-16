use loopagent_kernel::Kernel;
use loopagent_runtime::tool_pipeline::execute_tool;
use loopagent_types::config::Settings;
use loopagent_types::tool::ToolContext;
use std::path::PathBuf;

fn make_kernel() -> Kernel {
    Kernel::new(Settings::default()).expect("Kernel::new should succeed")
}

fn make_ctx() -> ToolContext {
    ToolContext {
        cwd: PathBuf::from("/tmp"),
        session_id: "test-session".to_string(),
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
        &loopagent_runtime::mode::AgentMode::Act,
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
    let test_file = tmp_dir.join("loopagent_test_tool_pipeline_read.txt");
    std::fs::write(&test_file, "hello from test").unwrap();

    let ctx = ToolContext {
        cwd: tmp_dir.clone(),
        session_id: "test-session".to_string(),
    };

    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": test_file.to_str().unwrap()}),
        &ctx,
        &loopagent_runtime::mode::AgentMode::Act,
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
        serde_json::json!({"file_path": "/tmp/nonexistent_file_loopagent_test_xyz_12345.txt"}),
        &ctx,
        &loopagent_runtime::mode::AgentMode::Act,
    )
    .await;

    if let Ok(r) = result { assert!(r.is_error, "reading missing file should set is_error=true") }
}

#[tokio::test]
async fn test_execute_tool_in_plan_mode() {
    let kernel = make_kernel();
    let tmp_dir = std::env::temp_dir();
    let test_file = tmp_dir.join("loopagent_test_tool_pipeline_plan.txt");
    std::fs::write(&test_file, "plan mode test content").unwrap();

    let ctx = ToolContext {
        cwd: tmp_dir.clone(),
        session_id: "test-session".to_string(),
    };

    let result = execute_tool(
        &kernel,
        "Read",
        serde_json::json!({"file_path": test_file.to_str().unwrap()}),
        &ctx,
        &loopagent_runtime::mode::AgentMode::Plan,
    )
    .await;

    let _ = std::fs::remove_file(&test_file);
    let result = result.expect("Read tool should succeed even in plan mode");
    assert!(!result.is_error);
}
