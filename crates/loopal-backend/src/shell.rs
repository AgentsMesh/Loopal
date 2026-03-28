//! Shell command execution with OS-level sandbox wrapping.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::ExecResult;
use loopal_tool_api::truncate_output;
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

    let stdout = truncate_output(
        &stdout_raw,
        limits.max_output_lines,
        limits.max_output_bytes,
    );
    let stderr = truncate_output(
        &stderr_raw,
        limits.max_output_lines,
        limits.max_output_bytes,
    );
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(ExecResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Spawn a background command under OS sandbox; returns a task ID.
///
/// Registers the task in the shared `loopal_tool_background` store so that
/// `Bash(process_id=...)` can query or stop it.
pub async fn exec_background(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    desc: &str,
) -> Result<String, ToolIoError> {
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;

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

    let task_id = loopal_tool_background::generate_task_id();
    let output_buf = Arc::new(Mutex::new(String::new()));
    let exit_code_buf = Arc::new(Mutex::new(None));
    let status_buf = Arc::new(Mutex::new(loopal_tool_background::TaskStatus::Running));

    let (watch_tx, watch_rx) =
        tokio::sync::watch::channel(loopal_tool_background::TaskStatus::Running);

    let task = loopal_tool_background::BackgroundTask {
        output: Arc::clone(&output_buf),
        exit_code: Arc::clone(&exit_code_buf),
        status: Arc::clone(&status_buf),
        description: desc.to_string(),
        child: Arc::new(Mutex::new(Some(child))),
        status_watch: watch_rx,
    };

    let child_handle = Arc::clone(&task.child);
    loopal_tool_background::store()
        .lock()
        .unwrap()
        .insert(task_id.clone(), task);

    let ob = Arc::clone(&output_buf);
    let eb = Arc::clone(&exit_code_buf);
    let sb = Arc::clone(&status_buf);
    tokio::spawn(async move {
        let mut child = child_handle.lock().unwrap().take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let (mut out_b, mut err_b) = (Vec::new(), Vec::new());
        let _ = tokio::join!(
            stdout.read_to_end(&mut out_b),
            stderr.read_to_end(&mut err_b)
        );
        let mut combined = String::from_utf8_lossy(&out_b).into_owned();
        if !err_b.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&String::from_utf8_lossy(&err_b));
        }
        *ob.lock().unwrap() = combined;
        let code = child.wait().await.ok().and_then(|s| s.code());
        *eb.lock().unwrap() = code;
        let final_status = if code == Some(0) {
            loopal_tool_background::TaskStatus::Completed
        } else {
            loopal_tool_background::TaskStatus::Failed
        };
        *sb.lock().unwrap() = final_status.clone();
        let _ = watch_tx.send(final_status);
    });

    Ok(task_id)
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
