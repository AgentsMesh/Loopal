//! Agent child process management — spawn, monitor, and clean up.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::{Child, Command};
use tracing::{info, warn};

use loopal_ipc::StdioTransport;
use loopal_ipc::transport::Transport;

/// Grace period before SIGKILL after requesting shutdown.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// A managed agent child process communicating over stdin/stdout.
pub struct AgentProcess {
    child: Child,
    transport: Arc<dyn Transport>,
}

impl AgentProcess {
    /// Spawn an agent worker process with additional environment variables.
    ///
    /// The child's stdin/stdout are captured for IPC. Stderr is inherited
    /// (passes through to the parent's terminal for debugging/logging).
    pub async fn spawn_with_env(
        executable: Option<&str>,
        env_vars: &[(&str, &str)],
    ) -> anyhow::Result<Self> {
        let exe = executable.unwrap_or("loopal");
        let exe_path = Self::resolve_executable(exe)?;

        info!(exe = %exe_path.display(), "spawning agent process");

        let mut cmd = Command::new(&exe_path);
        cmd.arg("--serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        let mut child = cmd.spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture child stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture child stdout"))?;

        let transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
            Box::new(tokio::io::BufReader::new(stdout)),
            Box::new(stdin),
        ));

        Ok(Self { child, transport })
    }

    /// Spawn an agent worker process.
    pub async fn spawn(executable: Option<&str>) -> anyhow::Result<Self> {
        Self::spawn_with_env(executable, &[]).await
    }

    /// Get the transport for creating an IPC `Connection`.
    pub fn transport(&self) -> Arc<dyn Transport> {
        self.transport.clone()
    }

    /// Get the child process ID (for monitoring / kill).
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Check if the child process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Wait for the child to exit (consumes self).
    pub async fn wait(mut self) -> std::io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }

    /// Graceful shutdown: close the transport writer (signals EOF to child),
    /// wait for exit, then SIGKILL if the grace period expires.
    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        info!("shutting down agent process");

        // Close the transport writer → child's transport.recv() returns None → server exits.
        // Note: child.stdin was already moved into the transport during spawn,
        // so we must close via the transport rather than dropping child.stdin.
        self.transport.close().await;

        // Wait with timeout for graceful exit
        match tokio::time::timeout(SHUTDOWN_GRACE, self.child.wait()).await {
            Ok(Ok(status)) => {
                info!(?status, "agent process exited gracefully");
            }
            Ok(Err(e)) => {
                warn!("error waiting for agent process: {e}");
            }
            Err(_) => {
                warn!("agent process did not exit within grace period, killing");
                if let Err(e) = self.child.kill().await {
                    warn!("failed to kill agent process: {e}");
                }
                let _ = self.child.wait().await;
            }
        }
        Ok(())
    }

    fn resolve_executable(name: &str) -> anyhow::Result<PathBuf> {
        // Check LOOPAL_BINARY env var first (set by Bazel or test harness).
        if let Ok(path) = std::env::var("LOOPAL_BINARY") {
            let p = PathBuf::from(&path);
            if p.exists() {
                return Ok(p);
            }
        }
        // If an explicit path is provided and exists, use it directly.
        let explicit = PathBuf::from(name);
        if explicit.is_absolute() && explicit.exists() {
            return Ok(explicit);
        }
        // Otherwise, use the current executable (same binary, worker mode).
        if let Ok(current) = std::env::current_exe()
            && current.exists()
        {
            return Ok(current);
        }
        Ok(explicit)
    }
}
