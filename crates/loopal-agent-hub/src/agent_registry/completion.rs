//! Agent completion tracking, result delivery, and cascade interrupt.

use std::collections::VecDeque;

use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentCompletion, AgentEvent, AgentEventPayload, Envelope, MessageSource, QualifiedAddress,
};
use tokio::sync::{mpsc, watch};

use super::AgentRegistry;
use crate::topology::AgentLifecycle;

/// Completion side effects that must leave the Hub lock before delivery.
///
/// Completion state and watcher notification are committed synchronously in
/// `AgentRegistry`. The ordered Error/Finished events retain bounded-channel
/// backpressure and are delivered by the caller without holding the Hub lock.
#[must_use = "completion events must be delivered after releasing the Hub lock"]
pub struct PendingCompletionDelivery {
    event_tx: mpsc::Sender<AgentEvent>,
    events: VecDeque<AgentEvent>,
    parent_delivery: Option<(mpsc::Sender<Envelope>, Envelope)>,
}

impl PendingCompletionDelivery {
    fn new(
        event_tx: mpsc::Sender<AgentEvent>,
        events: Vec<AgentEvent>,
        parent_delivery: Option<(mpsc::Sender<Envelope>, Envelope)>,
    ) -> Self {
        Self {
            event_tx,
            events: events.into(),
            parent_delivery,
        }
    }

    /// Enqueue every authoritative completion event in semantic order.
    pub async fn deliver_events(&mut self) -> Result<(), mpsc::error::SendError<()>> {
        while !self.events.is_empty() {
            // Reserve before removing the event. If this future is cancelled
            // while backpressured, the pending Error/Finished sequence remains
            // intact and can be retried by the owner.
            let permit = self.event_tx.reserve().await?;
            let event = self
                .events
                .pop_front()
                .expect("pending completion event disappeared after reserve");
            permit.send(event);
        }
        Ok(())
    }

    pub fn take_parent_delivery(&mut self) -> Option<(mpsc::Sender<Envelope>, Envelope)> {
        self.parent_delivery.take()
    }

    pub fn has_parent_delivery(&self) -> bool {
        self.parent_delivery.is_some()
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use std::time::Duration;

    use loopal_ipc::connection::Connection;

    use super::*;

    #[tokio::test]
    async fn cancelled_backpressure_retains_the_ordered_terminal_sequence() {
        let (event_tx, mut event_rx) = mpsc::channel(1);
        event_tx
            .send(AgentEvent::root(AgentEventPayload::Running))
            .await
            .unwrap();
        let mut registry = AgentRegistry::new(event_tx);
        let mut pending = registry.emit_agent_completion(
            "worker",
            AgentCompletion::new("error", Some("provider failed".into())),
        );

        assert!(
            tokio::time::timeout(Duration::from_millis(10), pending.deliver_events())
                .await
                .is_err(),
            "full event queue should backpressure completion delivery"
        );
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Running
        ));

        let delivery = pending.deliver_events();
        let receive = async {
            let error = event_rx.recv().await.unwrap();
            let finished = event_rx.recv().await.unwrap();
            (error, finished)
        };
        let (result, (error, finished)) = tokio::join!(delivery, receive);
        result.unwrap();
        assert!(matches!(
            error.payload,
            AgentEventPayload::Error { ref message } if message == "provider failed"
        ));
        assert!(matches!(finished.payload, AgentEventPayload::Finished));
        assert_eq!(error.routing_generation, finished.routing_generation);
    }

    #[tokio::test]
    async fn old_child_completion_cannot_reach_same_name_reconnected_parent() {
        let (event_tx, mut event_rx) = mpsc::channel(8);
        let mut registry = AgentRegistry::new(event_tx);
        let (_old_parent_peer, old_parent_transport) = loopal_ipc::duplex_pair();
        let (old_parent, _old_parent_incoming) =
            Connection::new(old_parent_transport).into_listening();
        let (old_completion_tx, _old_completion_rx) = mpsc::channel(1);
        registry
            .register_connection_with_parent(
                "parent",
                old_parent,
                None,
                None,
                Some(old_completion_tx),
            )
            .unwrap();
        let old_parent_generation = registry.generation("parent").unwrap();

        let (_child_peer, child_transport) = loopal_ipc::duplex_pair();
        let (child, _child_incoming) = Connection::new(child_transport).into_listening();
        registry
            .register_connection_with_parent(
                "child",
                child,
                Some(QualifiedAddress::local("parent")),
                None,
                None,
            )
            .unwrap();
        assert_eq!(
            registry.agents["child"].parent_generation,
            Some(old_parent_generation)
        );

        registry.unregister_connection("parent");
        let (_new_parent_peer, new_parent_transport) = loopal_ipc::duplex_pair();
        let (new_parent, _new_parent_incoming) =
            Connection::new(new_parent_transport).into_listening();
        let (new_completion_tx, mut new_completion_rx) = mpsc::channel(1);
        registry
            .register_connection_with_parent(
                "parent",
                new_parent,
                None,
                None,
                Some(new_completion_tx),
            )
            .unwrap();
        assert_ne!(registry.generation("parent"), Some(old_parent_generation));

        let mut pending = registry.emit_agent_completion(
            "child",
            AgentCompletion::goal(Some("old child result".into())),
        );
        assert!(!pending.has_parent_delivery());
        assert!(
            registry
                .local_parent_generation_for_completion("child")
                .is_none()
        );
        pending.deliver_events().await.unwrap();
        assert!(matches!(
            event_rx.recv().await.unwrap().payload,
            AgentEventPayload::Finished
        ));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), new_completion_rx.recv())
                .await
                .is_err(),
            "same-name replacement parent must not receive an older edge's completion"
        );
    }
}

impl AgentRegistry {
    /// Emit Finished event, cache typed completion, deliver it, and notify watchers.
    ///
    /// Returns all effects that require async delivery after releasing the Hub
    /// lock. This avoids both lock-held backpressure and lossy `try_send`.
    pub fn emit_agent_finished(
        &mut self,
        name: &str,
        output: Option<String>,
    ) -> PendingCompletionDelivery {
        self.emit_agent_completion(name, AgentCompletion::goal(output))
    }

    /// Record and fan out the authoritative `agent/completed` payload.
    pub fn emit_agent_completion(
        &mut self,
        name: &str,
        completion: AgentCompletion,
    ) -> PendingCompletionDelivery {
        if let Some(existing) = self.completion(name) {
            tracing::warn!(
                agent = %name,
                existing_reason = %existing.reason,
                ignored_reason = %completion.reason,
                "duplicate completion ignored"
            );
            return PendingCompletionDelivery::new(self.event_tx.clone(), Vec::new(), None);
        }
        tracing::info!(
            agent = %name,
            reason = %completion.reason,
            has_result = completion.result.is_some(),
            "emitting completion"
        );
        let was_failed = self
            .agent_info(name)
            .is_some_and(|info| matches!(info.lifecycle, AgentLifecycle::Failed(_)));
        let error_event_admitted = self.has_admitted_error(name);
        let synthetic_error = if completion.is_success() || was_failed || error_event_admitted {
            None
        } else {
            Some(completion.failure_detail().to_string())
        };
        if completion.is_success() {
            self.set_completion_lifecycle(name, AgentLifecycle::Finished);
        } else {
            let error = completion.failure_detail().to_string();
            self.set_completion_lifecycle(name, AgentLifecycle::Failed(error));
        }

        self.remember_completion(name, completion.clone());

        // Prepare delivery envelope (actual send happens after lock release).
        let pending_delivery = self.prepare_parent_delivery(name, &completion);

        let mut events = Vec::with_capacity(usize::from(synthetic_error.is_some()) + 1);
        if let Some(message) = synthetic_error {
            let event = AgentEvent::named(name, AgentEventPayload::Error { message });
            events.push(self.prepare_generation_event(name, event));
        }
        let event = AgentEvent::named(name, AgentEventPayload::Finished);
        events.push(self.prepare_generation_event(name, event));

        if let Some(tx) = self.completions.remove(name) {
            let _ = tx.send(Some(completion));
        }

        let orphans = self.collect_orphaned_children(name);
        if !orphans.is_empty() {
            tracing::info!(agent = %name, orphans = ?orphans, "cascade interrupt");
            self.interrupt_orphans(&orphans);
        }

        PendingCompletionDelivery::new(self.event_tx.clone(), events, pending_delivery)
    }

    /// Build the delivery envelope and find the parent's completion_tx.
    /// Returns None if no parent, parent is remote, or parent has no
    /// completion channel registered.
    fn prepare_parent_delivery(
        &self,
        child_name: &str,
        completion: &AgentCompletion,
    ) -> Option<(mpsc::Sender<Envelope>, Envelope)> {
        let child = self.agents.get(child_name)?;
        if !child.notify_parent_on_completion {
            return None;
        }
        let parent = child.info.parent.as_ref()?;
        // Local-only delivery path. Remote parents take the uplink route in
        // `finish::finish_and_deliver` instead.
        if parent.is_remote() {
            return None;
        }
        let expected_parent_generation = child.parent_generation?;
        let parent_agent = self.agents.get(&parent.agent)?;
        if parent_agent.generation != expected_parent_generation
            || parent_agent.info.lifecycle.is_terminal()
        {
            return None;
        }
        let tx = parent_agent.completion_tx.as_ref()?.clone();
        // Cap large results: save to overflow file, embed path in envelope.
        let result = completion.output();
        let body = if result.len() > MAX_RESULT_BYTES {
            overflow_agent_result(child_name, result)
        } else {
            result.to_string()
        };
        let delivered_completion = AgentCompletion::new(
            completion.reason.clone(),
            completion.result.as_ref().map(|_| body.clone()),
        );
        // Source carries the child's local view (uplink SNAT stamps the origin
        // hub on cross-hub delivery). The `<agent-result>` wrapper is rebuilt at
        // LLM projection time — the envelope body stays raw so observers render
        // it structurally.
        let envelope = Envelope::new(
            MessageSource::AgentResult {
                child: QualifiedAddress::local(child_name),
            },
            parent.clone(),
            body,
        )
        .with_agent_completion(delivered_completion);
        Some((tx, envelope))
    }

    pub(crate) fn notifies_parent_on_completion(&self, name: &str) -> bool {
        self.agents
            .get(name)
            .is_some_and(|agent| agent.notify_parent_on_completion)
    }

    pub(crate) fn local_parent_generation_for_completion(&self, name: &str) -> Option<u64> {
        let child = self.agents.get(name)?;
        if !child.notify_parent_on_completion {
            return None;
        }
        let parent = child
            .info
            .parent
            .as_ref()
            .filter(|parent| parent.is_local())?;
        let expected_generation = child.parent_generation?;
        self.agents
            .get(&parent.agent)
            .is_some_and(|agent| {
                agent.generation == expected_generation && !agent.info.lifecycle.is_terminal()
            })
            .then_some(expected_generation)
    }

    /// Create a completion watcher for a named agent.
    pub fn watch_completion(&mut self, name: &str) -> watch::Receiver<Option<AgentCompletion>> {
        if let Some(tx) = self.completions.get(name) {
            return tx.subscribe();
        }
        let initial = self.completion(name).cloned();
        let (tx, rx) = watch::channel(initial);
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
                crate::types::AgentConnectionState::Shadow => {
                    // Shadow entries represent remote agents — can't interrupt locally.
                    tracing::debug!(agent = %name, "skipping interrupt for shadow entry");
                }
            }
        }
    }

    pub(crate) fn collect_orphaned_children(&self, parent: &str) -> Vec<String> {
        self.agents
            .get(parent)
            .map_or_else(Vec::new, |parent_agent| {
                let parent_generation = parent_agent.generation;
                parent_agent
                    .info
                    .children
                    .iter()
                    .filter(|c| {
                        self.agents.get(c.as_str()).is_some_and(|child| {
                            child.parent_generation == Some(parent_generation)
                                && child.info.lifecycle == AgentLifecycle::Running
                                && !child.state.is_shadow()
                            // Shadows are remote and cannot be interrupted locally.
                        })
                    })
                    .cloned()
                    .collect()
            })
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

/// Max agent result bytes before overflow to file (100 KB).
const MAX_RESULT_BYTES: usize = 100_000;

/// Save oversized agent result to file, return preview + path.
fn overflow_agent_result(agent_name: &str, result: &str) -> String {
    let dir = std::env::temp_dir().join("loopal").join("overflow");
    let _ = std::fs::create_dir_all(&dir);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = dir.join(format!("agent_{agent_name}_{ts}.txt"));
    let path_str = path.to_string_lossy().into_owned();
    if std::fs::write(&path, result).is_err() {
        return result[..MAX_RESULT_BYTES].to_string();
    }
    // Preview: first ~25 KB
    let preview_end = result
        .char_indices()
        .take_while(|(i, _)| *i < MAX_RESULT_BYTES / 4)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let kb = result.len() / 1024;
    format!(
        "{}\n\n[Agent result too large for context ({kb} KB). Full output saved to: {path_str}]\n\
         Use the Read tool to access the complete output if needed.",
        &result[..preview_end]
    )
}
