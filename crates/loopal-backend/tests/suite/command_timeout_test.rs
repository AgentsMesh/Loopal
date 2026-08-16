use std::sync::Arc;
use std::time::Duration;

use loopal_backend::shell::exec_command;
use loopal_backend::shell_stream::{TimedOutProcessData, exec_command_streaming};
use loopal_tool_api::backend_types::EnvOverride;
use loopal_tool_api::{BgTaskConfig, ExecOutcome, OutputTail};

use super::log_file_test_support::unique_session_id;
#[cfg(unix)]
use crate::process_test_support::{
    remove_pid_file, unique_pid_path, wait_for_pid, wait_until_terminal,
};

#[tokio::test]
#[cfg(unix)]
async fn streaming_timeout_handle_carries_live_owner_and_log() {
    let tail = Arc::new(OutputTail::new(20));
    let outcome = exec_command_streaming(
        &std::env::temp_dir(),
        None,
        "echo PARTIAL; sleep 5",
        &EnvOverride::default(),
        Duration::from_millis(100),
        tail,
        &unique_session_id(),
    )
    .await
    .unwrap();
    let ExecOutcome::TimedOut { handle, .. } = outcome else {
        panic!("expected timeout");
    };
    let data = handle.0.downcast::<TimedOutProcessData>().unwrap();
    assert!(data.log_path.starts_with(std::env::temp_dir()));
    let log_path = data.log_path.clone();
    let TimedOutProcessData {
        mut spawned,
        capture_state,
        capture_task,
        ..
    } = *data;
    let config = BgTaskConfig::default();
    spawned.terminate(config.sigterm_grace()).await;
    loopal_backend::process_capture_task::join_bounded(capture_task, config.drainers_grace())
        .await
        .unwrap();
    assert!(log_path.is_file());
    assert!(!capture_state.capture_failed());
}

#[tokio::test]
#[cfg(unix)]
async fn foreground_timeout_kills_descendant() {
    let pid_file = unique_pid_path("timeout-tree");
    let command = format!(
        "(trap '' TERM; while :; do sleep 1; done) & child=$!; echo $child > '{}'; wait",
        pid_file.display()
    );
    let cwd = std::env::temp_dir();
    let env = EnvOverride::default();
    let session_id = unique_session_id();
    let result = exec_command(
        &cwd,
        None,
        &command,
        &env,
        Duration::from_millis(200),
        &session_id,
    )
    .await;
    assert!(matches!(result, Err(loopal_error::ToolIoError::Timeout(_))));
    let descendant = wait_for_pid(&pid_file).await;
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
}

#[tokio::test]
#[cfg(unix)]
async fn dropping_unadopted_timeout_handle_kills_descendant() {
    let pid_file = unique_pid_path("drop-timeout-tree");
    let command = format!(
        "(while :; do sleep 1; done) & child=$!; echo $child > '{}'; wait",
        pid_file.display()
    );
    let outcome = exec_command_streaming(
        &std::env::temp_dir(),
        None,
        &command,
        &EnvOverride::default(),
        Duration::from_millis(200),
        Arc::new(OutputTail::new(20)),
        &unique_session_id(),
    )
    .await
    .unwrap();
    let ExecOutcome::TimedOut { handle, .. } = outcome else {
        panic!("expected timeout");
    };
    let descendant = wait_for_pid(&pid_file).await;

    drop(handle);
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
}

#[tokio::test]
#[cfg(unix)]
async fn aborting_foreground_future_kills_descendant() {
    let pid_file = unique_pid_path("cancel-tree");
    let command = format!(
        "(while :; do sleep 1; done) & child=$!; echo $child > '{}'; wait",
        pid_file.display()
    );
    let session_id = unique_session_id();
    let task = tokio::spawn(async move {
        exec_command(
            &std::env::temp_dir(),
            None,
            &command,
            &EnvOverride::default(),
            Duration::from_secs(30),
            &session_id,
        )
        .await
    });
    let descendant = wait_for_pid(&pid_file).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    wait_until_terminal(descendant).await;
    remove_pid_file(&pid_file).await;
}
