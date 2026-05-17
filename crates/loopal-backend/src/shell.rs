use std::path::Path;
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{EnvOverride, ExecResult};
use tokio::task::AbortHandle;

use crate::log_writer::flush_writer;
use crate::process_group::{SpawnedChild, kill_process_group};
use crate::shell_spawn::{prepare_spawn, spawn_readers};

pub use crate::shell_spawn::SpawnedBackgroundData;

const EXEC_KILL_GRACE: Duration = Duration::from_millis(500);

pub async fn exec_command(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    timeout: Duration,
    session_id: &str,
) -> Result<ExecResult, ToolIoError> {
    let mut prepared = prepare_spawn(cwd, policy, command, env_overrides, session_id).await?;
    let readers = spawn_readers(
        prepared.stdout_pipe.take(),
        prepared.stderr_pipe.take(),
        prepared.log_writer.clone(),
        prepared.head_tail.clone(),
        prepared.stderr_buf.clone(),
        None,
    );

    let SpawnedChild { mut child, pgid } = prepared.spawned;

    let exit_code = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => status.code().unwrap_or(-1),
        Ok(Err(e)) => {
            for r in readers {
                r.abort();
            }
            return Err(ToolIoError::ExecFailed(format!("wait failed: {e}")));
        }
        Err(_) => {
            // reason: killpg the whole process group so grandchildren (sh fork
            // pnpm → node) don't leak past the timeout; kill_on_drop only
            // signals child PID.
            let _ = kill_process_group(pgid, &mut child, EXEC_KILL_GRACE).await;
            for r in readers {
                r.abort();
            }
            return Err(ToolIoError::Timeout(timeout));
        }
    };

    for r in readers {
        let _ = r.await;
    }
    flush_writer(&prepared.log_writer).await;

    let stdout = prepared.head_tail.render_preview();
    let stdout_truncated = prepared.head_tail.was_truncated();
    let (stderr, stderr_truncated) = {
        let g = prepared.stderr_buf.lock();
        (g.snapshot(), g.was_truncated())
    };
    Ok(ExecResult {
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
        exit_code,
        log_path: prepared.log_path,
    })
}

pub async fn exec_background(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    session_id: &str,
) -> Result<SpawnedBackgroundData, ToolIoError> {
    let mut prepared = prepare_spawn(cwd, policy, command, env_overrides, session_id).await?;
    let readers = spawn_readers(
        prepared.stdout_pipe.take(),
        prepared.stderr_pipe.take(),
        prepared.log_writer.clone(),
        prepared.head_tail.clone(),
        prepared.stderr_buf.clone(),
        None,
    );
    let drainers: Vec<AbortHandle> = readers.into_iter().map(|h| h.abort_handle()).collect();

    Ok(SpawnedBackgroundData {
        spawned: prepared.spawned,
        log_path: prepared.log_path,
        head_tail: prepared.head_tail,
        stderr_buf: prepared.stderr_buf,
        drainers,
    })
}
