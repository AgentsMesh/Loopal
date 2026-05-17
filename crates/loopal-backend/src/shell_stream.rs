use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::ExecOutcome;
use loopal_tool_api::backend_types::{EnvOverride, ExecResult};
use loopal_tool_api::{HeadTail, OutputTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;
use tokio::task::AbortHandle;

use crate::log_writer::flush_writer;
use crate::process_group::SpawnedChild;
use crate::shell_spawn::{prepare_spawn, spawn_readers};

pub struct TimedOutProcessData {
    pub spawned: SpawnedChild,
    pub log_path: std::path::PathBuf,
    pub stdout_head_tail: Arc<HeadTail>,
    pub stderr_buf: Arc<PlMutex<StderrCappedBuffer>>,
    pub abort_handles: Vec<AbortHandle>,
}

pub async fn exec_command_streaming(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    timeout: Duration,
    tail: Arc<OutputTail>,
    session_id: &str,
) -> Result<ExecOutcome, ToolIoError> {
    let mut prepared = prepare_spawn(cwd, policy, command, env_overrides, session_id).await?;
    let readers = spawn_readers(
        prepared.stdout_pipe.take(),
        prepared.stderr_pipe.take(),
        prepared.log_writer.clone(),
        prepared.head_tail.clone(),
        prepared.stderr_buf.clone(),
        Some(tail.clone()),
    );
    let abort_handles: Vec<AbortHandle> = readers.iter().map(|h| h.abort_handle()).collect();

    let SpawnedChild { mut child, pgid } = prepared.spawned;

    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => {
            for h in readers {
                let _ = h.await;
            }
            flush_writer(&prepared.log_writer).await;
            let exit_code = status.code().unwrap_or(-1);
            let stdout = prepared.head_tail.render_preview();
            let stdout_truncated = prepared.head_tail.was_truncated();
            let (stderr, stderr_truncated) = {
                let g = prepared.stderr_buf.lock();
                (g.snapshot(), g.was_truncated())
            };
            Ok(ExecOutcome::Completed(ExecResult {
                stdout,
                stderr,
                stdout_truncated,
                stderr_truncated,
                exit_code,
                log_path: prepared.log_path,
            }))
        }
        Ok(Err(e)) => {
            for h in readers {
                h.abort();
            }
            Err(ToolIoError::ExecFailed(format!("wait failed: {e}")))
        }
        Err(_timeout) => {
            // reason: detach readers (no abort) so they keep filling the log
            // until child closes pipes. AbortHandles travel to the monitor
            // inside TimedOutProcessData for cleanup.
            drop(readers);
            let partial = tail.snapshot();
            Ok(ExecOutcome::TimedOut {
                timeout,
                partial_output: partial,
                handle: ProcessHandle(Box::new(TimedOutProcessData {
                    spawned: SpawnedChild { child, pgid },
                    log_path: prepared.log_path,
                    stdout_head_tail: prepared.head_tail,
                    stderr_buf: prepared.stderr_buf,
                    abort_handles,
                })),
            })
        }
    }
}
