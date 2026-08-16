//! Agent completion tracking, result delivery, and cascade interrupt.

use std::collections::VecDeque;

use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentCompletion, AgentEvent, AgentEventPayload, Envelope, MessageSource, QualifiedAddress,
};
use tokio::sync::{mpsc, watch};

use super::AgentRegistry;
use crate::topology::AgentLifecycle;

#[cfg(test)]
#[path = "completion_guard_tests.rs"]
mod completion_guard_tests;

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
#[path = "completion_tests.rs"]
mod tests;

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
        let result_limit = self
            .current_execution(name)
            .map(|execution| self.completion_result_limit(&execution))
            .unwrap_or(loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES);
        let completion = crate::completion_guard::guard_with_result_limit(
            completion,
            &self.final_sink_redaction_seed,
            result_limit,
        );
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
        let body = completion.output().to_string();
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

    pub(crate) fn watch_completion_exact(
        &mut self,
        execution: &crate::types::AgentExecutionRef,
    ) -> Option<watch::Receiver<Option<AgentCompletion>>> {
        self.owns_lease(execution)
            .then(|| self.watch_completion(&execution.address.agent))
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
