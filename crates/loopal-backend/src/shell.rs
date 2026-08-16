use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::ProcessOutputSanitizer;
use loopal_tool_api::backend_types::{EnvOverride, ExecResult};

use crate::process_capture_result::exec_result;
use crate::process_capture_task;
use crate::process_cleanup::terminate_and_drain;
use crate::process_wait::{self, WaitOutcome};
use crate::shell_spawn::{CapturePolicy, prepare_spawn, spawn_capture};

pub use crate::shell_spawn::SpawnedBackgroundData;

pub async fn exec_command(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    timeout: Duration,
    session_id: &str,
) -> Result<ExecResult, ToolIoError> {
    exec_command_guarded(
        cwd,
        policy,
        command,
        env_overrides,
        timeout,
        session_id,
        None,
    )
    .await
}

pub async fn exec_command_guarded(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    timeout: Duration,
    session_id: &str,
    sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
) -> Result<ExecResult, ToolIoError> {
    let capture = CapturePolicy::new(session_id, sanitizer);
    let mut prepared = prepare_spawn(cwd, policy, command, env_overrides, &capture).await?;
    let capture_task = spawn_capture(&mut prepared, None, capture.sanitizer);
    let wait = process_wait::wait(&mut prepared.spawned, &prepared.capture_state, timeout).await;

    match wait {
        WaitOutcome::Exited(Ok(status)) => {
            process_capture_task::join(capture_task).await?;
            Ok(exec_result(
                &prepared.capture_state,
                status.code().unwrap_or(-1),
                prepared.log_path,
            ))
        }
        WaitOutcome::Exited(Err(error)) => {
            terminate_and_drain(&mut prepared.spawned, capture_task).await?;
            Err(ToolIoError::ExecFailed(format!("wait failed: {error}")))
        }
        WaitOutcome::CaptureFailed => {
            terminate_and_drain(&mut prepared.spawned, capture_task).await?;
            Err(ToolIoError::ExecFailed(
                "process output capture failed".into(),
            ))
        }
        WaitOutcome::TimedOut => {
            terminate_and_drain(&mut prepared.spawned, capture_task).await?;
            Err(ToolIoError::Timeout(timeout))
        }
    }
}

pub async fn exec_background(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    session_id: &str,
) -> Result<SpawnedBackgroundData, ToolIoError> {
    exec_background_guarded(cwd, policy, command, env_overrides, session_id, None).await
}

pub async fn exec_background_guarded(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    session_id: &str,
    sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
) -> Result<SpawnedBackgroundData, ToolIoError> {
    let capture = CapturePolicy::new(session_id, sanitizer);
    let mut prepared = prepare_spawn(cwd, policy, command, env_overrides, &capture).await?;
    let capture_task = spawn_capture(&mut prepared, None, capture.sanitizer);
    Ok(SpawnedBackgroundData {
        spawned: prepared.spawned,
        log_path: prepared.log_path,
        capture_state: prepared.capture_state,
        capture_task,
    })
}
