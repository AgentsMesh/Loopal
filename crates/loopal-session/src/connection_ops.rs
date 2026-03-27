//! Attach/detach/reattach operations for AgentConnectionManager.

use std::sync::Arc;

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::AgentEvent;
use tracing::info;

use crate::connection_manager::{
    AgentConnectionManager, AgentConnectionState, AttachedConn, ManagedAgent,
};

impl AgentConnectionManager {
    /// Attach to a sub-agent via TCP. Spawns a background task that reads
    /// events from the sub-agent and feeds them into the shared event_tx.
    pub async fn attach(&mut self, name: &str, port: u16, token: &str) -> anyhow::Result<()> {
        let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .map_err(|e| anyhow::anyhow!("TCP connect to sub-agent {name}: {e}"))?;
        let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
        let conn = Arc::new(Connection::new(transport));
        let mut rx = conn.start();

        // Initialize with token
        conn.send_request(
            "initialize",
            serde_json::json!({"protocol_version": 1, "token": token}),
        )
        .await
        .map_err(|e| anyhow::anyhow!("initialize sub-agent {name}: {e}"))?;

        // Join the active session to receive event broadcasts
        let join_result = conn
            .send_request(
                loopal_ipc::protocol::methods::AGENT_JOIN.name,
                serde_json::json!({}),
            )
            .await;
        if let Err(e) = join_result {
            tracing::warn!(agent = name, error = %e, "agent/join failed, events may not flow");
        }

        // Spawn event reader task
        let event_tx = self.event_tx.clone();
        let agent_name = name.to_string();
        let event_task = tokio::spawn(async move {
            read_agent_events(&mut rx, &event_tx, &agent_name).await;
        });

        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Attached(AttachedConn {
                    connection: conn,
                    event_task,
                    port,
                    token: token.to_string(),
                }),
            },
        );
        info!(agent = name, port, "attached to sub-agent");
        Ok(())
    }

    /// Detach from a sub-agent. Keeps port/token for re-attach. Agent keeps running.
    pub fn detach(&mut self, name: &str) {
        if let Some(agent) = self.agents.get_mut(name) {
            if let AgentConnectionState::Attached(conn) = &agent.state {
                let port = conn.port;
                let token = conn.token.clone();
                conn.event_task.abort();
                agent.state = AgentConnectionState::Detached { port, token };
                info!(agent = name, "detached from sub-agent");
            }
        }
    }

    /// Re-attach to a previously detached sub-agent.
    pub async fn reattach(&mut self, name: &str) -> anyhow::Result<()> {
        let (port, token) = match self.agents.get(name) {
            Some(ManagedAgent {
                state: AgentConnectionState::Detached { port, token },
            }) => (*port, token.clone()),
            _ => anyhow::bail!("agent {name} is not detached"),
        };
        self.attach(name, port, &token).await
    }

    /// Handle SubAgentSpawned event — auto-attach to the new sub-agent.
    pub async fn on_sub_agent_spawned(&mut self, name: &str, _pid: u32, port: u16, token: &str) {
        if let Err(e) = self.attach(name, port, token).await {
            tracing::warn!(agent = name, error = %e, "failed to auto-attach");
        }
    }

    /// Send interrupt to a specific agent.
    pub async fn interrupt(&self, name: &str) {
        if let Some(agent) = self.agents.get(name) {
            match &agent.state {
                AgentConnectionState::Primary(conn) => {
                    conn.interrupt.signal();
                    conn.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                }
                AgentConnectionState::Attached(conn) => {
                    let _ = conn
                        .connection
                        .send_notification(methods::AGENT_INTERRUPT.name, serde_json::json!({}))
                        .await;
                }
                AgentConnectionState::Detached { .. } => {}
            }
        }
    }

    /// Check if an agent is in a given state.
    pub fn is_attached(&self, name: &str) -> bool {
        self.agents
            .get(name)
            .is_some_and(|a| matches!(a.state, AgentConnectionState::Attached(_)))
    }

    /// List all agents with their connection state labels.
    pub fn list_agents(&self) -> Vec<(String, &'static str)> {
        self.agents
            .iter()
            .map(|(name, agent)| {
                let label = match &agent.state {
                    AgentConnectionState::Primary(_) => "primary",
                    AgentConnectionState::Attached(_) => "attached",
                    AgentConnectionState::Detached { .. } => "detached",
                };
                (name.clone(), label)
            })
            .collect()
    }
}

/// Background task: read agent/event notifications and forward to TUI.
async fn read_agent_events(
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    agent_name: &str,
) {
    while let Some(msg) = rx.recv().await {
        if let Incoming::Notification { method, params } = msg {
            if method == methods::AGENT_EVENT.name {
                if let Ok(mut event) = serde_json::from_value::<AgentEvent>(params) {
                    // Ensure agent_name is set for proper TUI routing
                    if event.agent_name.is_none() {
                        event.agent_name = Some(agent_name.to_string());
                    }
                    if event_tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
    info!(agent = agent_name, "sub-agent event stream ended");
}
