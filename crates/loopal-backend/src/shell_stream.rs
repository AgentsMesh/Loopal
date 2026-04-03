//! Streaming shell command execution — spawn + line-by-line stdout capture.
//!
//! Feeds real-time output into an `OutputTail` ring buffer so the progress
//! reporter can include actual command output in `ToolProgress` events.
//!
//! On timeout the child is **not** killed — instead an
//! [`ExecOutcome::TimedOut`] is returned so the caller can register it as
//! a background task.

use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::ExecOutcome;
use loopal_tool_api::backend_types::ExecResult;
use loopal_tool_api::handle_overflow;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::task::AbortHandle;

use crate::limits::ResourceLimits;
use crate::shell::build_command;
use loopal_tool_api::OutputTail;

/// Data attached to [`ExecOutcome::TimedOut`] on timeout.
///
/// The child is wrapped in `Arc<Mutex<Option<Child>>>` so that a
/// background-monitor task can take ownership later.
pub struct TimedOutProcessData {
    pub child: Arc<Mutex<Option<Child>>>,
    pub stdout_buf: Arc<Mutex<String>>,
    pub stderr_buf: Arc<Mutex<String>>,
    /// Abort handles for reader tasks — call `.abort()` after the child
    /// exits if they haven't finished draining.
    pub abort_handles: Vec<AbortHandle>,
}

/// Execute a shell command with streaming output capture.
///
/// Like `exec_command`, but spawns the process and reads stdout/stderr line by
/// line, pushing each line into the `tail` buffer for real-time observation.
///
/// Returns [`ExecOutcome::TimedOut`] when the timeout is exceeded (the process
/// is **not** killed).  The caller can downcast the [`ProcessHandle`] to
/// [`TimedOutProcessData`] and register it as a background task.
pub async fn exec_command_streaming(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    timeout: Duration,
    limits: &ResourceLimits,
    tail: Arc<OutputTail>,
) -> Result<ExecOutcome, ToolIoError> {
    let (program, args, env) = build_command(cwd, policy, command);

    let mut cmd = tokio::process::Command::new(&program);
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

    let mut child = cmd
        .spawn()
        .map_err(|e| ToolIoError::ExecFailed(format!("spawn failed: {e}")))?;

    let stdout_pipe = child.stdout.take().unwrap();
    let stderr_pipe = child.stderr.take().unwrap();

    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let child_arc = Arc::new(Mutex::new(Some(child)));

    let ob = Arc::clone(&stdout_buf);
    let eb = Arc::clone(&stderr_buf);
    let stdout_tail = Arc::clone(&tail);

    let stdout_task =
        tokio::spawn(
            async move { read_lines_into_buf(stdout_pipe, &ob, Some(&stdout_tail)).await },
        );
    let stderr_task =
        tokio::spawn(async move { read_lines_into_buf(stderr_pipe, &eb, None).await });

    // Grab abort handles BEFORE moving JoinHandles into the timeout block.
    // If timeout fires, the JoinHandles are dropped (detaching the tasks),
    // but abort handles survive and are stored in TimedOutProcessData for
    // cleanup by the monitor.
    let stdout_abort = stdout_task.abort_handle();
    let stderr_abort = stderr_task.abort_handle();

    // Readers + child wait all inside the timeout.  This ensures `take()`
    // only runs after pipes close (readers finish → child exited), so on
    // timeout the child is still inside child_arc (not taken/dropped).
    let child_for_wait = Arc::clone(&child_arc);
    let wait_result = tokio::time::timeout(timeout, async {
        let (r1, r2) = tokio::join!(stdout_task, stderr_task);
        let _ = (r1, r2);
        let child_opt = child_for_wait.lock().unwrap().take();
        if let Some(mut ch) = child_opt {
            let status = ch
                .wait()
                .await
                .map_err(|e| ToolIoError::ExecFailed(format!("wait failed: {e}")))?;
            Ok::<_, ToolIoError>(status.code().unwrap_or(-1))
        } else {
            Ok(-1)
        }
    })
    .await;

    match wait_result {
        Ok(Ok(exit_code)) => {
            let result = build_exec_result(&stdout_buf, &stderr_buf, exit_code, limits)?;
            Ok(ExecOutcome::Completed(result))
        }
        Ok(Err(e)) => Err(e),
        Err(_timeout) => {
            let partial = tail.snapshot();
            Ok(ExecOutcome::TimedOut {
                timeout,
                partial_output: partial,
                handle: ProcessHandle(Box::new(TimedOutProcessData {
                    child: child_arc,
                    stdout_buf,
                    stderr_buf,
                    abort_handles: vec![stdout_abort, stderr_abort],
                })),
            })
        }
    }
}

/// Build the final `ExecResult` from shared buffers after successful completion.
fn build_exec_result(
    stdout_buf: &Mutex<String>,
    stderr_buf: &Mutex<String>,
    exit_code: i32,
    limits: &ResourceLimits,
) -> Result<ExecResult, ToolIoError> {
    let stdout_raw = stdout_buf.lock().unwrap().clone();
    let stderr_raw = stderr_buf.lock().unwrap().clone();
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
    Ok(ExecResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Read lines from an async reader into a shared buffer, optionally pushing
/// to an OutputTail.
async fn read_lines_into_buf<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    buf: &Mutex<String>,
    tail: Option<&OutputTail>,
) {
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                if let Some(t) = tail {
                    t.push_line(line.trim_end_matches('\n').to_string());
                }
                buf.lock().unwrap().push_str(&line);
            }
            Err(_) => break,
        }
    }
}
