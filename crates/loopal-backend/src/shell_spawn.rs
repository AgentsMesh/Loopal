use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_protocol::META_HUB_TOKEN_ENV;
use loopal_tool_api::backend_types::EnvOverride;
use loopal_tool_api::{OutputTail, ProcessOutputSanitizer};
use tokio::process::{ChildStderr, ChildStdout, Command};

use crate::log_writer::{LogWriter, create_log_file};
use crate::process_capture;
use crate::process_capture_state::ProcessCaptureState;
use crate::process_capture_task::ProcessCaptureTask;
use crate::process_group::SpawnedChild;

pub(crate) const HEAD_LINES: usize = 25;
pub(crate) const TAIL_LINES: usize = 25;

pub(crate) struct CapturePolicy<'a> {
    pub(crate) session_id: &'a str,
    pub(crate) sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
}

impl<'a> CapturePolicy<'a> {
    pub(crate) fn new(
        session_id: &'a str,
        sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
    ) -> Self {
        Self {
            session_id,
            sanitizer,
        }
    }
}

pub struct SpawnedBackgroundData {
    pub spawned: SpawnedChild,
    pub log_path: std::path::PathBuf,
    pub capture_state: Arc<ProcessCaptureState>,
    pub capture_task: ProcessCaptureTask,
}

pub(crate) struct PreparedSpawn {
    pub spawned: SpawnedChild,
    pub stdout_pipe: Option<ChildStdout>,
    pub stderr_pipe: Option<ChildStderr>,
    pub log_path: std::path::PathBuf,
    pub log_writer: Arc<LogWriter>,
    pub capture_state: Arc<ProcessCaptureState>,
}

pub(crate) async fn prepare_spawn(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    capture: &CapturePolicy<'_>,
) -> Result<PreparedSpawn, ToolIoError> {
    let (log_path, log_writer) = create_log_file(capture.session_id).await?;
    let capture_state = ProcessCaptureState::new(log_path.clone(), capture.sanitizer.clone());
    let (program, args, env) = build_command(cwd, policy, command);
    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(env_map) = env {
        cmd.env_clear();
        for (key, value) in env_map {
            cmd.env(key, value);
        }
    }
    for (key, value) in &env_overrides.vars {
        cmd.env(key, value);
    }
    cmd.env_remove(META_HUB_TOKEN_ENV);

    let mut spawned = SpawnedChild::spawn(cmd)
        .map_err(|error| ToolIoError::ExecFailed(format!("spawn failed: {error}")))?;
    let stdout_pipe = spawned.take_stdout();
    let stderr_pipe = spawned.take_stderr();
    Ok(PreparedSpawn {
        spawned,
        stdout_pipe,
        stderr_pipe,
        log_path,
        log_writer: Arc::new(log_writer),
        capture_state,
    })
}

pub(crate) fn spawn_capture(
    prepared: &mut PreparedSpawn,
    progress: Option<Arc<OutputTail>>,
    sanitizer: Option<Arc<dyn ProcessOutputSanitizer>>,
) -> ProcessCaptureTask {
    process_capture::spawn(
        prepared.stdout_pipe.take(),
        prepared.stderr_pipe.take(),
        prepared.log_writer.clone(),
        prepared.capture_state.clone(),
        progress,
        sanitizer,
    )
}

#[cfg(test)]
#[path = "shell_spawn_tests.rs"]
mod tests;

type EnvMap = std::collections::HashMap<String, String>;

pub(crate) fn build_command(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
) -> (String, Vec<String>, Option<EnvMap>) {
    if let Some(policy) = policy {
        let command = loopal_sandbox::command_wrapper::wrap_command(policy, command, cwd);
        (command.program, command.args, Some(command.env))
    } else if cfg!(windows) {
        let comspec = std::env::var("COMSPEC").unwrap_or("cmd.exe".into());
        (comspec, vec!["/C".into(), command.into()], None)
    } else {
        ("sh".into(), vec!["-c".into(), command.into()], None)
    }
}
