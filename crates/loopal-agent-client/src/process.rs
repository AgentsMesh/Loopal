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
    _stderr_drain: Option<tokio::task::JoinHandle<()>>,
}

impl AgentProcess {
    /// Spawn an agent worker process with additional environment variables.
    ///
    /// The child's stdin/stdout are captured for IPC. Stderr is piped and
    /// drained to tracing to avoid corrupting the parent TUI terminal.
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
            .stderr(Stdio::piped())
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

        let stderr_drain = child
            .stderr
            .take()
            .map(|stderr| tokio::spawn(crate::stderr_drain::drain_to_tracing(stderr)));

        let transport: Arc<dyn Transport> = Arc::new(StdioTransport::new(
            Box::new(tokio::io::BufReader::new(stdout)),
            Box::new(stdin),
        ));

        Ok(Self {
            child,
            transport,
            _stderr_drain: stderr_drain,
        })
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

    /// Wait for the child to exit with a timeout, then SIGKILL if it doesn't.
    ///
    /// Intended for callers that need a bounded wait after signalling the child
    /// to exit (e.g. after closing the transport). Not currently used by the
    /// default spawn path — kept as a utility for future callers.
    pub async fn wait_or_kill(mut self, timeout: Duration) {
        match tokio::time::timeout(timeout, self.child.wait()).await {
            Ok(Ok(status)) => {
                info!(?status, "agent child exited");
            }
            Ok(Err(e)) => {
                warn!("error waiting for agent child: {e}");
            }
            Err(_) => {
                warn!("agent child did not exit within grace period, killing");
                if let Err(e) = self.child.kill().await {
                    warn!("failed to kill agent child: {e}");
                }
                let _ = self.child.wait().await;
            }
        }
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
