use loopal_tool_api::Tool;
use loopal_tool_bash::BashTool;
use serde_json::json;

use super::{make_ctx, make_store};

#[tokio::test]
async fn test_bash_simple_echo() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"command": "echo hello"}), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("hello"));
}

#[tokio::test]
async fn test_bash_nonzero_exit_code() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"command": "exit 42"}), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("exit_code: 42"));
}

#[tokio::test]
async fn test_bash_missing_command_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({}), &ctx).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_bash_captures_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(json!({"command": "echo 'err msg' >&2"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("err msg"));
}

#[tokio::test]
async fn test_bash_stdout_and_stderr_combined() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"command": "echo stdout_out; echo stderr_out >&2"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("stdout_out"));
    assert!(result.content.contains("stderr_out"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn test_bash_runs_in_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool.execute(json!({"command": "pwd"}), &ctx).await.unwrap();

    assert!(!result.is_error);
    let output = result.content.trim();
    let canon_tmp = tmp.path().canonicalize().unwrap();
    let canon_output = std::path::PathBuf::from(output).canonicalize().unwrap();
    assert_eq!(canon_output, canon_tmp);
}

#[tokio::test]
async fn test_bash_with_custom_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "command": "echo fast",
                "timeout": 30
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("fast"));
}

#[tokio::test]
async fn test_bash_timeout_triggers_error() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({
                "command": "sleep 60",
                "timeout": 0
            }),
            &ctx,
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
#[cfg(not(windows))]
async fn test_bash_command_with_nonzero_exit_and_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = BashTool::new(make_store());
    let ctx = make_ctx(tmp.path());

    let result = tool
        .execute(
            json!({"command": "echo 'failure output' >&2; exit 1"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("exit_code: 1"));
    assert!(result.content.contains("failure output"));
}
