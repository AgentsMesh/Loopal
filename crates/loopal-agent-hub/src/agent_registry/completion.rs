//! Agent completion tracking and cascade interrupt.

use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use tokio::sync::watch;

use super::AgentRegistry;
use crate::topology::AgentLifecycle;

impl AgentRegistry {
    /// Emit Finished event, cache output, notify watchers, cascade orphans.
    pub fn emit_agent_finished(&mut self, name: &str, output: Option<String>) {
        tracing::info!(agent = %name, has_output = output.is_some(), "emitting Finished");
        self.set_lifecycle(name, AgentLifecycle::Finished);

        let text = output.unwrap_or_else(|| "(no output)".into());
        self.finished_outputs.insert(name.to_string(), text.clone());

        let event = AgentEvent::named(name, AgentEventPayload::Finished);
        let _ = self.event_tx.try_send(event);

        if let Some(tx) = self.completions.remove(name) {
            let _ = tx.send(Some(text));
        }

        let orphans = self.collect_orphaned_children(name);
        if !orphans.is_empty() {
            tracing::info!(agent = %name, orphans = ?orphans, "cascade interrupt");
            self.interrupt_orphans(&orphans);
        }
    }

    /// Create a completion watcher for a named agent.
    pub fn watch_completion(&mut self, name: &str) -> watch::Receiver<Option<String>> {
        let (tx, rx) = watch::channel(None);
        self.completions.insert(name.to_string(), tx);
        rx
    }

    /// Send interrupt to a specific agent.
    pub async fn interrupt(&self, name: &str) {
        if let Some(agent) = self.agents.get(name) {
            match &agent.state {
                crate::types::AgentConnectionState::Local(ch) => {
                    ch.interrupt.signal();
                    ch.interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                }
                crate::types::AgentConnectionState::Connected(conn) => {
                    let _ = conn
                        .send_notification(methods::AGENT_INTERRUPT.name, serde_json::json!({}))
                        .await;
                }
            }
        }
    }

    pub(crate) fn collect_orphaned_children(&self, parent: &str) -> Vec<String> {
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

    pub(crate) fn interrupt_orphans(&self, orphans: &[String]) {
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
