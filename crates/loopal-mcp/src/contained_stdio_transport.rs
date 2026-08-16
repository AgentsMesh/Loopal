use std::io;
use std::process::Stdio;
use std::time::Duration;

use loopal_backend::{KillOutcome, SpawnedChild};
use rmcp::RoleClient;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::transport::async_rw::AsyncRwTransport;
use tokio::process::{ChildStderr, ChildStdin, ChildStdout, Command};

const EXIT_GRACE: Duration = Duration::from_millis(500);

pub(crate) struct ContainedStdioTransport {
    io: AsyncRwTransport<RoleClient, ChildStdout, ChildStdin>,
    child: Option<SpawnedChild>,
}

impl ContainedStdioTransport {
    pub(crate) fn spawn(mut command: Command) -> io::Result<(Self, Option<ChildStderr>)> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = SpawnedChild::spawn(command)?;
        let stdin = child
            .take_stdin()
            .ok_or_else(|| io::Error::other("MCP stdio stdin unavailable"))?;
        let stdout = child
            .take_stdout()
            .ok_or_else(|| io::Error::other("MCP stdio stdout unavailable"))?;
        let stderr = child.take_stderr();
        Ok((
            Self {
                io: AsyncRwTransport::new(stdout, stdin),
                child: Some(child),
            },
            stderr,
        ))
    }

    async fn close_child(&mut self) -> io::Result<()> {
        self.io.close().await?;
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        match tokio::time::timeout(EXIT_GRACE, child.wait()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(_)) => Err(io::Error::other("MCP stdio containment wait failed")),
            Err(_) => match child.terminate(EXIT_GRACE).await.outcome {
                KillOutcome::KillFailed(_) => {
                    Err(io::Error::other("MCP stdio containment termination failed"))
                }
                _ => Ok(()),
            },
        }
    }
}

impl Transport<RoleClient> for ContainedStdioTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        item: ClientJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        self.io.send(item)
    }

    async fn receive(&mut self) -> Option<ServerJsonRpcMessage> {
        self.io.receive().await
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        self.close_child().await
    }
}

#[cfg(test)]
#[path = "contained_stdio_transport_tests.rs"]
mod tests;
