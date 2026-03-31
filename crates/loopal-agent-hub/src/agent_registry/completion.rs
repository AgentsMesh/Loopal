//! Agent completion tracking, result delivery, and cascade interrupt.

use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, MessageSource};
use tokio::sync::{mpsc, watch};

use super::AgentRegistry;
use crate::topology::AgentLifecycle;

impl AgentRegistry {
    /// Emit Finished event, cache output, deliver result to parent, notify watchers.
    ///
    /// Returns an optional `(sender, envelope)` pair for the caller to deliver
    /// **after releasing the Hub lock**. This avoids holding the lock during IPC.
    pub fn emit_agent_finished(
        &mut self,
        name: &str,
        output: Option<String>,
    ) -> Option<(mpsc::Sender<Envelope>, Envelope)> {
        tracing::info!(agent = %name, has_output = output.is_some(), "emitting Finished");
        self.set_lifecycle(name, AgentLifecycle::Finished);

        let text = output.unwrap_or_else(|| "(no output)".into());
        self.finished_outputs.insert(name.to_string(), text.clone());

        // Prepare delivery envelope (actual send happens after lock release).
        let pending_delivery = self.prepare_parent_delivery(name, &text);

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

        pending_delivery
    }

    /// Build the delivery envelope and find the parent's completion_tx.
    /// Returns None if no parent or no completion channel.
    fn prepare_parent_delivery(
        &self,
        child_name: &str,
        result: &str,
    ) -> Option<(mpsc::Sender<Envelope>, Envelope)> {
        let parent_name = self.agents.get(child_name)?.info.parent.as_deref()?;
        let tx = self
            .agents
            .get(parent_name)?
            .completion_tx
            .as_ref()?
            .clone();
        let content = format!("<agent-result name=\"{child_name}\">\n{result}\n</agent-result>");
        let envelope = Envelope::new(
            MessageSource::System("agent-completed".into()),
            parent_name,
            content,
        );
        Some((tx, envelope))
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
