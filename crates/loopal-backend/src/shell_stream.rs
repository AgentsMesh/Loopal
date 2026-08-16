use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::backend_types::{EnvOverride, ExecResult};
use loopal_tool_api::{ExecOutcome, OutputTail};

use crate::process_capture_result::exec_result;
use crate::process_capture_task;
use crate::process_cleanup::terminate_and_drain;
use crate::process_group::SpawnedChild;
use crate::process_wait::{self, WaitOutcome};
use crate::shell_spawn::{CapturePolicy, prepare_spawn, spawn_capture};
use crate::{ProcessCaptureState, ProcessCaptureTask};

pub struct TimedOutProcessData {
    pub spawned: SpawnedChild,
    pub log_path: std::path::PathBuf,
    pub capture_state: Arc<ProcessCaptureState>,
    pub capture_task: ProcessCaptureTask,
}

fn wait_failed(error: &std::io::Error) -> ToolIoError {
    ToolIoError::ExecFailed(format!("wait failed: {error}"))
}

fn capture_failed() -> ToolIoError {
    ToolIoError::ExecFailed("process output capture failed".into())
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
    exec_command_streaming_guarded(
        cwd,
        policy,
        command,
        env_overrides,
        timeout,
        tail,
        CapturePolicy::new(session_id, None),
    )
    .await
}

pub(crate) async fn exec_command_streaming_guarded(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    timeout: Duration,
    tail: Arc<OutputTail>,
    capture: CapturePolicy<'_>,
) -> Result<ExecOutcome, ToolIoError> {
    let mut prepared = prepare_spawn(cwd, policy, command, env_overrides, &capture).await?;
    let capture_task = spawn_capture(&mut prepared, Some(tail.clone()), capture.sanitizer);
    let wait = process_wait::wait(&mut prepared.spawned, &prepared.capture_state, timeout).await;

    match wait {
        WaitOutcome::Exited(Ok(status)) => {
            process_capture_task::join(capture_task).await?;
            let result: ExecResult = exec_result(
                &prepared.capture_state,
                status.code().unwrap_or(-1),
                prepared.log_path,
            );
            Ok(ExecOutcome::Completed(result))
        }
        WaitOutcome::Exited(Err(error)) => {
            terminate_and_drain(&mut prepared.spawned, capture_task).await?;
            Err(wait_failed(&error))
        }
        WaitOutcome::CaptureFailed => {
            terminate_and_drain(&mut prepared.spawned, capture_task).await?;
            Err(capture_failed())
        }
        WaitOutcome::TimedOut => Ok(ExecOutcome::TimedOut {
            timeout,
            partial_output: tail.snapshot(),
            handle: ProcessHandle(Box::new(TimedOutProcessData {
                spawned: prepared.spawned,
                log_path: prepared.log_path,
                capture_state: prepared.capture_state,
                capture_task,
            })),
        }),
    }
}

#[cfg(test)]
#[path = "shell_stream_tests.rs"]
mod tests;
