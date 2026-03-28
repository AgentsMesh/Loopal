//! Connection operations for AgentHub.

use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEventPayload;

use crate::hub::AgentHub;
use crate::topology::AgentLifecycle;
use crate::types::AgentConnectionState;

impl AgentHub {
    /// Send interrupt to a specific agent.
    pub async fn interrupt(&self, name: &str) {
        if let Some(agent) = self.agents.get(name) {
            match &agent.state {
                AgentConnectionState::Local(ch) => {
                    ch.interrupt.signal();
                    ch.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                }
                AgentConnectionState::Connected(conn) => {
                    let _ = conn
                        .send_notification(methods::AGENT_INTERRUPT.name, serde_json::json!({}))
                        .await;
                }
            }
        }
    }

    /// List all agents with their connection state labels.
    pub fn list_agents(&self) -> Vec<(String, &'static str)> {
        self.agents
            .iter()
            .map(|(name, agent)| {
                let label = match &agent.state {
                    AgentConnectionState::Local(_) => "local",
                    AgentConnectionState::Connected(_) => "connected",
                };
                (name.clone(), label)
            })
            .collect()
    }

    /// Emit Finished event, cache output, update lifecycle, notify watchers.
    pub fn emit_agent_finished(&mut self, name: &str, output: Option<String>) {
        tracing::info!(agent = %name, has_output = output.is_some(), "emitting Finished event");
        self.set_lifecycle(name, AgentLifecycle::Finished);

        let text = output.unwrap_or_else(|| "(no output)".into());
        // Cache output for late wait_agent callers (race-safe)
        self.finished_outputs.insert(name.to_string(), text.clone());

        let event = loopal_protocol::AgentEvent::named(name, AgentEventPayload::Finished);
        let _ = self.event_tx.try_send(event);

        // Notify anyone already waiting on hub/wait_agent
        if let Some(tx) = self.completions.remove(name) {
            tracing::info!(agent = %name, "notifying completion watcher");
            let _ = tx.send(Some(text));
        }

        // Cascade: interrupt orphaned children via their IPC connections
        let orphans = self.collect_orphaned_children(name);
        if !orphans.is_empty() {
            tracing::info!(agent = %name, orphans = ?orphans, "cascade interrupt orphans");
            self.interrupt_orphans(&orphans);
        }
    }

    /// Create a completion watcher for a named agent.
    pub fn watch_completion(&mut self, name: &str) -> tokio::sync::watch::Receiver<Option<String>> {
        let (tx, rx) = tokio::sync::watch::channel(None);
        self.completions.insert(name.to_string(), tx);
        rx
    }

    /// Collect children that are still running when parent finishes.
    fn collect_orphaned_children(&self, parent: &str) -> Vec<String> {
        self.agents
            .get(parent)
            .map(|a| {
                a.info
                    .children
                    .iter()
                    .filter(|c| {
                        self.agents
                            .get(c.as_str())
                            .is_some_and(|a| a.info.lifecycle == AgentLifecycle::Running)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Send interrupt to orphaned agents via their IPC Connection.
    /// This is reliable — goes directly through the pipe, not through events.
    fn interrupt_orphans(&self, orphans: &[String]) {
        for name in orphans {
            if let Some(conn) = self.get_agent_connection(name) {
                let conn = conn.clone();
                let n = name.clone();
                tokio::spawn(async move {
                    let _ = conn
                        .send_notification(methods::AGENT_INTERRUPT.name, serde_json::json!({}))
                        .await;
                    tracing::info!(agent = %n, "sent interrupt to orphan");
                });
            }
        }
    }
}
