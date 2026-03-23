//! Streaming shell command execution — spawn + line-by-line stdout capture.
//!
//! Feeds real-time output into an `OutputTail` ring buffer so the progress
//! reporter can include actual command output in `ToolProgress` events.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::ExecResult;
use loopal_tool_api::truncate_output;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::limits::ResourceLimits;
use loopal_tool_api::OutputTail;
use crate::shell::build_command;

/// Execute a shell command with streaming output capture.
///
/// Like `exec_command`, but spawns the process and reads stdout/stderr line by
/// line, pushing each line into the `tail` buffer for real-time observation.
pub async fn exec_command_streaming(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    timeout_ms: u64,
    limits: &ResourceLimits,
    tail: Arc<OutputTail>,
) -> Result<ExecResult, ToolIoError> {
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

    let mut child = cmd.spawn()
        .map_err(|e| ToolIoError::ExecFailed(format!("spawn failed: {e}")))?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout_tail = Arc::clone(&tail);
    let stdout_task = tokio::spawn(async move {
        read_lines_into(stdout, Some(&stdout_tail)).await
    });
    let stderr_task = tokio::spawn(async move {
        read_lines_into(stderr, None).await
    });

    // Wait with timeout
    let result = tokio::time::timeout(Duration::from_millis(timeout_ms), async {
        let (stdout_res, stderr_res) = tokio::join!(stdout_task, stderr_task);
        let stdout_buf = stdout_res.unwrap_or_default();
        let stderr_buf = stderr_res.unwrap_or_default();
        let status = child.wait().await
            .map_err(|e| ToolIoError::ExecFailed(format!("wait failed: {e}")))?;
        Ok::<_, ToolIoError>((stdout_buf, stderr_buf, status.code().unwrap_or(-1)))
    })
    .await
    .map_err(|_| {
        let _ = child.start_kill();
        ToolIoError::Timeout(timeout_ms)
    })??;

    let (stdout_buf, stderr_buf, exit_code) = result;
    let stdout = truncate_output(&stdout_buf, limits.max_output_lines, limits.max_output_bytes);
    let stderr = truncate_output(&stderr_buf, limits.max_output_lines, limits.max_output_bytes);

    Ok(ExecResult { stdout, stderr, exit_code })
}

/// Read lines from an async reader, optionally pushing to an OutputTail.
async fn read_lines_into<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    tail: Option<&OutputTail>,
) -> String {
    let mut buf_reader = BufReader::new(reader);
    let mut output = String::new();
    let mut line = String::new();

    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                if let Some(t) = tail {
                    t.push_line(line.trim_end_matches('\n').to_string());
                }
                output.push_str(&line);
            }
            Err(_) => break,
        }
    }
    output
}
