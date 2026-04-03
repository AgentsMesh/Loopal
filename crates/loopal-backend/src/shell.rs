//! Shell command execution with OS-level sandbox wrapping.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::ExecResult;
use loopal_tool_api::handle_overflow;
use tokio::process::Command;

use crate::limits::ResourceLimits;

/// Execute a shell command with timeout and output truncation.
///
/// When `policy` is present, wraps the command with OS-level sandbox
/// (Seatbelt on macOS, bwrap on Linux). Otherwise runs plain `sh -c`.
pub async fn exec_command(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    timeout_ms: u64,
    limits: &ResourceLimits,
) -> Result<ExecResult, ToolIoError> {
    let (program, args, env) = build_command(cwd, policy, command);

    let mut cmd = Command::new(&program);
    cmd.args(&args).current_dir(cwd);
    if let Some(env_map) = env {
        cmd.env_clear();
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    let output = tokio::time::timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .map_err(|_| ToolIoError::Timeout(timeout_ms))?
        .map_err(|e| ToolIoError::ExecFailed(format!("spawn failed: {e}")))?;

    let stdout_raw = String::from_utf8_lossy(&output.stdout);
    let stderr_raw = String::from_utf8_lossy(&output.stderr);

    let stdout = handle_overflow(
        &stdout_raw,
        limits.max_output_lines,
        limits.max_output_bytes,
        "bash_stdout",
    )
    .display;
    let stderr = handle_overflow(
        &stderr_raw,
        limits.max_output_lines,
        limits.max_output_bytes,
        "bash_stderr",
    )
    .display;
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(ExecResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Spawn a background command under OS sandbox.
///
/// Returns a [`SpawnedBackgroundData`] containing the child process.
/// The caller is responsible for registering it in the background task
/// store and monitoring its lifecycle.
pub async fn exec_background(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
) -> Result<SpawnedBackgroundData, ToolIoError> {
    use std::process::Stdio;

    let (program, args, env) = build_command(cwd, policy, command);

    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(env_map) = env {
        cmd.env_clear();
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    let child = cmd
        .spawn()
        .map_err(|e| ToolIoError::ExecFailed(e.to_string()))?;

    Ok(SpawnedBackgroundData {
        child: Arc::new(Mutex::new(Some(child))),
    })
}

/// Data returned by [`exec_background`] — a spawned child ready for
/// background-store registration.
pub struct SpawnedBackgroundData {
    pub child: Arc<Mutex<Option<tokio::process::Child>>>,
}

type EnvMap = std::collections::HashMap<String, String>;

pub(crate) fn build_command(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
) -> (String, Vec<String>, Option<EnvMap>) {
    if let Some(pol) = policy {
        let sc = loopal_sandbox::command_wrapper::wrap_command(pol, command, cwd);
        (sc.program, sc.args, Some(sc.env))
    } else if cfg!(windows) {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
        (comspec, vec!["/C".into(), command.into()], None)
    } else {
        ("sh".into(), vec!["-c".into(), command.into()], None)
    }
}
