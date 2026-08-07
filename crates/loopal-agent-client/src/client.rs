//! IPC client — wraps `Connection` with agent protocol methods.

use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentCompletion, AgentEvent, ControlCommand, Envelope};

use crate::start_params::{StartAgentParams, encode};

/// High-level agent IPC client.
pub struct AgentClient {
    connection: Arc<Connection<Listening>>,
    incoming_rx: mpsc::Receiver<Incoming>,
}

impl AgentClient {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        let (connection, incoming_rx) = Connection::new(transport).into_listening();
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

    /// Send `agent/start` to begin the agent loop using the client's own
    /// connection and a 30s default timeout. Callers needing a different
    /// budget or a hand-owned `Connection` (e.g. after `into_parts`) should
    /// use [`AgentClient::start_agent_on`].
    pub async fn start_agent(&self, p: &StartAgentParams) -> anyhow::Result<String> {
        // reason: 30s covers process spawn + initialize + bounded MCP wait +
        // margin. Hub bootstrap shortens this via start_agent_on to keep
        // proxy(8s) < start_agent(20s) < HANDSHAKE(30s) layering intact.
        const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
        Self::start_agent_on(&self.connection, p, DEFAULT_TIMEOUT).await
    }

    /// Send `agent/start` over a caller-supplied `Connection` with a
    /// caller-supplied deadline. Stays in `AgentClient` so the RPC wire
    /// shape (`agent/start` + `encode(p)` + `session_id` extraction) lives
    /// in exactly one place.
    pub async fn start_agent_on(
        connection: &Connection<Listening>,
        p: &StartAgentParams,
        timeout: std::time::Duration,
    ) -> anyhow::Result<String> {
        let result = tokio::time::timeout(
            timeout,
            connection.send_request(methods::AGENT_START.name, encode(p)),
        )
        .await
        .map_err(|_| anyhow::anyhow!("agent/start timed out after {}s", timeout.as_secs()))?
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
                            Ok(event) => {
                                return Some(AgentClientEvent::AgentEvent(Box::new(event)));
                            }
                            Err(e) => tracing::warn!("failed to parse agent event: {e}"),
                        }
                    } else if method == methods::AGENT_COMPLETED.name {
                        match serde_json::from_value::<AgentCompletion>(params) {
                            Ok(completion) => {
                                return Some(AgentClientEvent::AgentCompleted(completion));
                            }
                            Err(e) => tracing::warn!("failed to parse agent completion: {e}"),
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
    pub fn into_parts(self) -> (Arc<Connection<Listening>>, mpsc::Receiver<Incoming>) {
        (self.connection, self.incoming_rx)
    }
}

/// Events received from the agent process.
#[derive(Debug)]
pub enum AgentClientEvent {
    /// An agent event (stream text, tool calls, status, etc).
    AgentEvent(Box<AgentEvent>),
    /// The authoritative terminal result from `agent/completed`.
    AgentCompleted(AgentCompletion),
}
