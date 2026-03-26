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
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// A managed agent child process communicating over stdin/stdout.
pub struct AgentProcess {
    child: Child,
    transport: Arc<dyn Transport>,
}

impl AgentProcess {
    /// Spawn `loopal --serve` as a child process.
    ///
    /// The child's stdin/stdout are captured for IPC. Stderr is inherited
    /// (passes through to the parent's terminal for debugging/logging).
    pub async fn spawn(executable: Option<&str>) -> anyhow::Result<Self> {
        let exe = executable.unwrap_or("loopal");
        let exe_path = Self::resolve_executable(exe)?;

        info!(exe = %exe_path.display(), "spawning agent process");

        let mut child = Command::new(&exe_path)
            .arg("--serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;

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

    /// Graceful shutdown: close stdin (signals EOF to child), wait for exit,
    /// then SIGKILL if the grace period expires.
    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        info!("shutting down agent process");

        // Close stdin → child's transport.recv() returns None → server loop exits
        drop(self.child.stdin.take());

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
        if let Ok(current) = std::env::current_exe() {
            if current.exists() {
                return Ok(current);
            }
        }
        Ok(PathBuf::from(name))
    }
}
