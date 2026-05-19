//! IPC client — wraps `Connection` with agent protocol methods.

use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope};

use crate::start_params::{StartAgentParams, encode};

/// High-level agent IPC client.
pub struct AgentClient {
    connection: Arc<Connection>,
    incoming_rx: mpsc::Receiver<Incoming>,
}

impl AgentClient {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        let connection = Arc::new(Connection::new(transport));
        let incoming_rx = connection.start();
        Self {
            connection,
            incoming_rx,
        }
    }

    /// Send `initialize` and wait for response.
    /// Retries on transient failures (e.g. agent process still starting up).
    pub async fn initialize(&self) -> anyhow::Result<Value> {
        use std::time::Duration;
        const MAX_ATTEMPTS: u32 = 5;
        // Sandboxed child-process spawn can need ~0.8–2 s on macOS just for
        // fork+exec + tracing init. Pair the longer per-attempt budget with
        // the idempotent server-side `initialize` handler so retries are safe.
        const TIMEOUT: Duration = Duration::from_secs(10);

        for attempt in 1..=MAX_ATTEMPTS {
            match tokio::time::timeout(
                TIMEOUT,
                self.connection.send_request(
                    methods::INITIALIZE.name,
                    serde_json::json!({"protocol_version": 1}),
                ),
            )
            .await
            {
                Ok(Ok(result)) => {
                    info!("IPC initialized: {result}");
                    return Ok(result);
                }
                Ok(Err(e)) if attempt < MAX_ATTEMPTS => {
                    tracing::warn!(attempt, error = %e, "initialize failed, retrying");
                    tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                }
                Ok(Err(e)) => {
                    return Err(anyhow::anyhow!(
                        "initialize failed after {MAX_ATTEMPTS} attempts: {e}"
                    ));
                }
                Err(_) if attempt < MAX_ATTEMPTS => {
                    tracing::warn!(attempt, "initialize timed out, retrying");
                    tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                }
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "initialize timed out after {MAX_ATTEMPTS} attempts"
                    ));
                }
            }
        }
        unreachable!()
    }

    /// Send `agent/start` to begin the agent loop.
    pub async fn start_agent(&self, p: &StartAgentParams) -> anyhow::Result<String> {
        // reason: parent hub_spawn waits 30s for handshake; cap below that so
        // user sees the real IPC error (e.g. session-not-found) instead of the
        // generic "handshake timeout".
        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
        let result = tokio::time::timeout(
            TIMEOUT,
            self.connection
                .send_request(methods::AGENT_START.name, encode(p)),
        )
        .await
        .map_err(|_| anyhow::anyhow!("agent/start timed out after {}s", TIMEOUT.as_secs()))?
        .map_err(|e| anyhow::anyhow!("agent/start failed: {e}"))?;
        let session_id = result["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("agent/start response missing session_id: {result}"))?
            .to_string();
        info!(session_id = %session_id, "agent started");
        Ok(session_id)
    }

    pub async fn send_message(&self, envelope: &Envelope) -> anyhow::Result<()> {
        let params = serde_json::to_value(envelope)?;
        self.connection
            .send_request(methods::AGENT_MESSAGE.name, params)
            .await
            .map_err(|e| anyhow::anyhow!("agent/message failed: {e}"))?;
        Ok(())
    }

    pub async fn send_control(&self, cmd: &ControlCommand) -> anyhow::Result<()> {
        let params = serde_json::to_value(cmd)?;
        self.connection
            .send_request(methods::AGENT_CONTROL.name, params)
            .await
            .map_err(|e| anyhow::anyhow!("agent/control failed: {e}"))?;
        Ok(())
    }

    pub async fn send_interrupt(&self) -> anyhow::Result<()> {
        self.connection
            .send_notification(methods::AGENT_INTERRUPT.name, Value::Null)
            .await
            .map_err(|e| anyhow::anyhow!("agent/interrupt failed: {e}"))?;
        Ok(())
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let _ = self
            .connection
            .send_request(methods::AGENT_SHUTDOWN.name, Value::Null)
            .await;
        Ok(())
    }

    /// Receive the next incoming message. Returns `None` when the connection closes.
    pub async fn recv(&mut self) -> Option<AgentClientEvent> {
        loop {
            let incoming = self.incoming_rx.recv().await?;
            match incoming {
                Incoming::Notification { method, params } => {
                    if method == methods::AGENT_EVENT.name {
                        match serde_json::from_value::<AgentEvent>(params) {
                            Ok(event) => return Some(AgentClientEvent::AgentEvent(event)),
                            Err(e) => tracing::warn!("failed to parse agent event: {e}"),
                        }
                    }
                }
                Incoming::Request { id, method, .. } => {
                    let _ = self
                        .connection
                        .respond_error(
                            id,
                            loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
                            &format!("agent client does not handle: {method}"),
                        )
                        .await;
                }
            }
        }
    }

    /// Check if the underlying connection is alive.
    pub fn is_connected(&self) -> bool {
        self.connection.is_connected()
    }

    /// Decompose into Connection + incoming receiver for bridge handoff.
    pub fn into_parts(self) -> (Arc<Connection>, mpsc::Receiver<Incoming>) {
        (self.connection, self.incoming_rx)
    }
}

/// Events received from the agent process.
#[derive(Debug)]
pub enum AgentClientEvent {
    /// An agent event (stream text, tool calls, status, etc).
    AgentEvent(AgentEvent),
}
