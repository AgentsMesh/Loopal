use std::time::Duration;

use loopal_config::{HookConfig, HookResult};
use loopal_error::HookError;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

/// Execute a hook command, passing `stdin_data` as JSON to its stdin.
pub async fn run_hook(
    config: &HookConfig,
    stdin_data: serde_json::Value,
) -> Result<HookResult, HookError> {
    debug!(command = %config.command, "running hook");

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&config.command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;

    // Write JSON to stdin — ignore BrokenPipe (child may exit without reading).
    if let Some(mut stdin) = child.stdin.take() {
        let data = serde_json::to_vec(&stdin_data)
            .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;
        if let Err(e) = stdin.write_all(&data).await
            && e.kind() != std::io::ErrorKind::BrokenPipe
        {
            return Err(HookError::ExecutionFailed(e.to_string()));
        }
        drop(stdin);
    }

    let timeout = Duration::from_millis(config.timeout_ms);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| HookError::Timeout(format!("hook timed out after {}ms", config.timeout_ms)))?
        .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;

    Ok(HookResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
