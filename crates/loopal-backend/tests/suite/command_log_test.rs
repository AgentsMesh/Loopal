use std::sync::Arc;
use std::time::Duration;

use loopal_backend::shell::{SpawnedBackgroundData, exec_background, exec_command};
use loopal_backend::shell_stream::exec_command_streaming;
use loopal_tool_api::ExecOutcome;
use loopal_tool_api::OutputTail;
use loopal_tool_api::backend_types::EnvOverride;

use super::log_file_test_support::unique_session_id;

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_writes_log_file_and_returns_path() {
    let cwd = std::env::temp_dir();
    let result = exec_command(
        &cwd,
        None,
        "echo SHORT_OUT",
        &EnvOverride::default(),
        Duration::from_secs(5),
        &unique_session_id(),
    )
    .await
    .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("SHORT_OUT"));
    assert_eq!(result.stderr, "");
    assert!(!result.stdout_truncated);
    assert!(!result.stderr_truncated);
    assert!(result.log_path.starts_with(std::env::temp_dir()));
    let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
    assert!(on_disk.contains("SHORT_OUT"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_interleaves_stderr_with_err_prefix_in_file() {
    let cwd = std::env::temp_dir();
    let result = exec_command(
        &cwd,
        None,
        "echo OUT_LINE; echo ERR_LINE >&2",
        &EnvOverride::default(),
        Duration::from_secs(5),
        &unique_session_id(),
    )
    .await
    .unwrap();
    assert!(result.stdout.contains("OUT_LINE"));
    assert!(result.stderr.contains("ERR_LINE"));
    let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
    assert!(on_disk.contains("OUT_LINE"));
    assert!(on_disk.contains("[err] ERR_LINE"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_truncated_flags_for_long_stdout() {
    let cwd = std::env::temp_dir();
    let cmd = r#"for i in $(seq 1 200); do echo "L$i"; done"#;
    let result = exec_command(
        &cwd,
        None,
        cmd,
        &EnvOverride::default(),
        Duration::from_secs(10),
        &unique_session_id(),
    )
    .await
    .unwrap();
    assert!(result.stdout_truncated);
    assert!(result.stdout.contains("L1"));
    assert!(result.stdout.contains("L200"));
    assert!(result.stdout.contains("lines elided"));
    let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
    assert!(on_disk.lines().count() >= 200);
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_background_creates_log_path_in_spawned_data() {
    let cwd = std::env::temp_dir();
    let data = exec_background(
        &cwd,
        None,
        "sleep 0.5",
        &EnvOverride::default(),
        &unique_session_id(),
    )
    .await
    .unwrap();
    let SpawnedBackgroundData {
        spawned,
        log_path,
        head_tail: _,
        stderr_buf: _,
        drainers: _,
    } = data;
    assert!(log_path.starts_with(std::env::temp_dir()));
    assert!(tokio::fs::metadata(&log_path).await.is_ok());
    let mut child = spawned.child;
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_streaming_completed_path_returns_log_path() {
    let cwd = std::env::temp_dir();
    let tail = Arc::new(OutputTail::new(20));
    let outcome = exec_command_streaming(
        &cwd,
        None,
        "echo STREAM_DONE",
        &EnvOverride::default(),
        Duration::from_secs(5),
        tail,
        &unique_session_id(),
    )
    .await
    .unwrap();
    match outcome {
        ExecOutcome::Completed(result) => {
            assert!(result.stdout.contains("STREAM_DONE"));
            assert!(result.log_path.starts_with(std::env::temp_dir()));
            let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
            assert!(on_disk.contains("STREAM_DONE"));
        }
        ExecOutcome::TimedOut { .. } => panic!("expected completed, got timeout"),
    }
}
