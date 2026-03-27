//! Attach/detach/reattach operations for AgentHub.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::AgentEvent;
use tracing::info;

use crate::hub::AgentHub;
use crate::types::{AgentConnectionState, AttachedConn, ManagedAgent};

const ATTACH_TIMEOUT: Duration = Duration::from_secs(5);

impl AgentHub {
    /// Attach to a sub-agent via TCP. Spawns a background task that reads
    /// events from the sub-agent and feeds them into the shared event_tx.
    ///
    /// If an agent with the same name is already attached, it is detached
    /// first to avoid orphaned event reader tasks.
    pub async fn attach(&mut self, name: &str, port: u16, token: &str) -> anyhow::Result<()> {
        // Clean up existing connection for this name (race protection).
        self.detach(name);

        let stream = tokio::time::timeout(
            ATTACH_TIMEOUT,
            tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")),
        )
        .await
        .map_err(|_| anyhow::anyhow!("TCP connect to sub-agent {name}: timeout"))?
        .map_err(|e| anyhow::anyhow!("TCP connect to sub-agent {name}: {e}"))?;

        let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
        let conn = Arc::new(Connection::new(transport));
        let mut rx = conn.start();

        conn.send_request(
            "initialize",
            serde_json::json!({"protocol_version": 1, "token": token}),
        )
        .await
        .map_err(|e| anyhow::anyhow!("initialize sub-agent {name}: {e}"))?;

        let join_result = conn
            .send_request(methods::AGENT_JOIN.name, serde_json::json!({}))
            .await;
        if let Err(e) = join_result {
            tracing::warn!(agent = name, error = %e, "agent/join failed");
        }

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

    /// Detach from a sub-agent. Keeps port/token for re-attach.
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

    /// Detach all sub-agents and abort their event reader tasks.
    /// Primary connections are intentionally skipped — they are managed
    /// by the bootstrap/process lifecycle, not the hub event loop.
    pub fn detach_all(&mut self) {
        let names: Vec<String> = self.agents.keys().cloned().collect();
        for name in names {
            self.detach(&name);
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

    /// Check if an agent is attached.
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

/// Background task: read agent/event notifications and forward to hub.
async fn read_agent_events(
    rx: &mut tokio::sync::mpsc::Receiver<Incoming>,
    event_tx: &tokio::sync::mpsc::Sender<AgentEvent>,
    agent_name: &str,
) {
    while let Some(msg) = rx.recv().await {
        if let Incoming::Notification { method, params } = msg {
            if method == methods::AGENT_EVENT.name {
                if let Ok(mut event) = serde_json::from_value::<AgentEvent>(params) {
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
