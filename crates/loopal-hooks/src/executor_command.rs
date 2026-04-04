//! Command hook executor — spawns a shell subprocess.
//!
//! Refactored from `runner.rs`. Implements `HookExecutor` trait (OCP).

use std::time::Duration;

use loopal_error::HookError;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

use crate::executor::{HookExecutor, RawHookOutput};

/// Executes a hook by spawning `sh -c <command>` and piping JSON to stdin.
pub struct CommandExecutor {
    pub command: String,
    pub timeout: Duration,
}

#[async_trait::async_trait]
impl HookExecutor for CommandExecutor {
    async fn execute(&self, input: serde_json::Value) -> Result<RawHookOutput, HookError> {
        debug!(command = %self.command, "running command hook");

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;

        // Write JSON to stdin — ignore BrokenPipe (child may exit without reading).
        if let Some(mut stdin) = child.stdin.take() {
            let data = serde_json::to_vec(&input)
                .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;
            if let Err(e) = stdin.write_all(&data).await
                && e.kind() != std::io::ErrorKind::BrokenPipe
            {
                return Err(HookError::ExecutionFailed(e.to_string()));
            }
            drop(stdin);
        }

        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| {
                HookError::Timeout(format!("hook timed out after {}ms", self.timeout.as_millis()))
            })?
            .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;

        Ok(RawHookOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
