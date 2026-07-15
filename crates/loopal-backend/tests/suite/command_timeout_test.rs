use std::sync::Arc;
use std::time::Duration;

use loopal_backend::shell::exec_command;
use loopal_backend::shell_stream::{TimedOutProcessData, exec_command_streaming};
use loopal_tool_api::ExecOutcome;
use loopal_tool_api::OutputTail;
use loopal_tool_api::backend_types::EnvOverride;

use super::log_file_test_support::unique_session_id;

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_streaming_timeout_handle_carries_log_path() {
    let cwd = std::env::temp_dir();
    let tail = Arc::new(OutputTail::new(20));
    let outcome = exec_command_streaming(
        &cwd,
        None,
        "echo PARTIAL; sleep 5",
        &EnvOverride::default(),
        Duration::from_millis(500),
        tail,
        &unique_session_id(),
    )
    .await
    .unwrap();
    match outcome {
        ExecOutcome::TimedOut { handle, .. } => {
            let data = handle.0.downcast::<TimedOutProcessData>().ok().unwrap();
            assert!(data.log_path.starts_with(std::env::temp_dir()));
            let TimedOutProcessData {
                spawned,
                stdout_head_tail,
                drainers,
                ..
            } = *data;
            let loopal_backend::SpawnedChild { mut child, pgid } = spawned;
            let config = loopal_tool_api::BgTaskConfig::default();
            let _ =
                loopal_backend::kill_process_group(pgid, &mut child, config.sigterm_grace()).await;
            let _ = child.wait().await;

            let aborts: Vec<_> = drainers.iter().map(|h| h.abort_handle()).collect();
            let drain = async move {
                for drainer in drainers {
                    let _ = drainer.await;
                }
            };
            if tokio::time::timeout(config.drainers_grace(), drain)
                .await
                .is_err()
            {
                for abort in aborts {
                    abort.abort();
                }
                panic!("output drainers did not finish after child exit");
            }
            let captured = stdout_head_tail.render_preview();
            assert!(
                captured.contains("PARTIAL"),
                "head_tail must contain echoed PARTIAL, got: {captured:?}"
            );
        }
        ExecOutcome::Completed(_) => panic!("expected timeout, got completed"),
    }
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_timeout_kills_process_group() {
    let cwd = std::env::temp_dir();
    let probe = uuid_lite();
    let pid_file = std::env::temp_dir().join(format!("loopal_pgid_{probe}"));
    let cmd = format!(
        "echo $$ > {} ; sleep 30 & sleep 30 & wait",
        pid_file.display()
    );
    let result = exec_command(
        &cwd,
        None,
        &cmd,
        &EnvOverride::default(),
        Duration::from_millis(400),
        &unique_session_id(),
    )
    .await;
    assert!(matches!(result, Err(loopal_error::ToolIoError::Timeout(_))));

    let pgid: i32 = match tokio::fs::read_to_string(&pid_file).await {
        Ok(s) => s.trim().parse().unwrap_or(0),
        Err(_) => return,
    };
    if pgid == 0 {
        return;
    }

    tokio::time::sleep(Duration::from_millis(700)).await;
    let count = count_pids_in_pgroup(pgid);
    assert_eq!(
        count, 0,
        "expected no process in pgid {pgid} after timeout, got {count}"
    );
    let _ = tokio::fs::remove_file(&pid_file).await;
}

fn uuid_lite() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

#[cfg(not(windows))]
fn count_pids_in_pgroup(pgid: i32) -> usize {
    let out = match std::process::Command::new("ps")
        .args(["-o", "pgid="])
        .arg("-g")
        .arg(pgid.to_string())
        .output()
    {
        Ok(o) => o,
        Err(_) => return 0,
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}
