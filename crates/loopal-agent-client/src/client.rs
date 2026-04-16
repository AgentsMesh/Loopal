//! IPC client — wraps `Connection` with agent protocol methods.

use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, ControlCommand, Envelope, UserQuestionResponse};

/// Parameters for `agent/start` IPC request.
#[derive(Debug, Default)]
pub struct StartAgentParams {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub prompt: Option<String>,
    pub permission_mode: Option<String>,
    pub no_sandbox: bool,
    pub resume: Option<String>,
    pub lifecycle: Option<String>,
    pub agent_type: Option<String>,
    /// Nesting depth (0 = root). Propagated from parent.
    pub depth: Option<u32>,
    /// Compressed parent conversation for fork context inheritance.
    pub fork_context: Option<serde_json::Value>,
}

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
        const TIMEOUT: Duration = Duration::from_secs(2);

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
        let mut params = serde_json::json!({
            "cwd": p.cwd.to_string_lossy(),
            "model": p.model,
            "mode": p.mode,
            "prompt": p.prompt,
            "permission_mode": p.permission_mode,
            "no_sandbox": p.no_sandbox,
            "resume": p.resume,
            "lifecycle": p.lifecycle,
            "agent_type": p.agent_type,
            "depth": p.depth,
        });
        if let Some(ref fc) = p.fork_context {
            params["fork_context"] = fc.clone();
        }
        let result = self
            .connection
            .send_request(methods::AGENT_START.name, params)
            .await
            .map_err(|e| anyhow::anyhow!("agent/start failed: {e}"))?;
        let session_id = result["session_id"]
            .as_str()
            .unwrap_or("unknown")
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
                Incoming::Request { id, method, params } => {
                    if method == methods::AGENT_PERMISSION.name {
                        return Some(AgentClientEvent::PermissionRequest { id, params });
                    }
                    if method == methods::AGENT_QUESTION.name {
                        return Some(AgentClientEvent::QuestionRequest { id, params });
                    }
                    // Unknown request — respond with error
                    let _ = self
                        .connection
                        .respond_error(
                            id,
                            loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
                            &format!("unknown method: {method}"),
                        )
                        .await;
                }
            }
        }
    }

    /// Respond to a permission request.
    pub async fn respond_permission(&self, request_id: i64, allow: bool) -> anyhow::Result<()> {
        self.connection
            .respond(request_id, serde_json::json!({"allow": allow}))
            .await
            .map_err(|e| anyhow::anyhow!("permission response failed: {e}"))
    }

    /// Respond to a question request.
    pub async fn respond_question(
        &self,
        request_id: i64,
        response: &UserQuestionResponse,
    ) -> anyhow::Result<()> {
        let value = serde_json::to_value(response)?;
        self.connection
            .respond(request_id, value)
            .await
            .map_err(|e| anyhow::anyhow!("question response failed: {e}"))
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
    /// The agent requests tool permission from the client.
    PermissionRequest { id: i64, params: Value },
    /// The agent asks a question to the user.
    QuestionRequest { id: i64, params: Value },
}
