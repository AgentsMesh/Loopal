//! Connection-generation-aware event admission.

use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload};

use super::AgentRegistry;
use crate::types::AgentExecutionRef;

impl AgentRegistry {
    /// Bind an inbound event to the exact connection generation that admitted
    /// it. The event may sit in the shared queue across a reconnect; the event
    /// router validates the generation again before reducing or broadcasting.
    pub(crate) fn prepare_connection_event(
        &mut self,
        name: &str,
        connection: &Arc<Connection<Listening>>,
        mut event: AgentEvent,
    ) -> Option<AgentEvent> {
        let agent = self.agents.get_mut(name)?;
        if !agent
            .state
            .connection()
            .is_some_and(|current| Arc::ptr_eq(&current, connection))
        {
            return None;
        }
        match &event.payload {
            AgentEventPayload::Error { .. } => agent.admitted_error = true,
            AgentEventPayload::Running | AgentEventPayload::Started => {
                agent.admitted_error = false;
            }
            _ => {}
        }
        event.routing_generation = Some(agent.generation);
        Some(event)
    }

    /// Bind a hub-synthesized event to the currently authoritative
    /// registration or retained tombstone generation. The caller must enqueue
    /// the returned event after releasing the Hub lock.
    pub(crate) fn prepare_generation_event(&self, name: &str, mut event: AgentEvent) -> AgentEvent {
        event.routing_generation = self.generation(name);
        event
    }

    pub(crate) fn prepare_execution_event(
        &self,
        execution: &AgentExecutionRef,
        mut event: AgentEvent,
    ) -> Option<AgentEvent> {
        if !self.owns_lease(execution) {
            return None;
        }
        event.routing_generation = Some(execution.connection_generation);
        Some(event)
    }

    pub(crate) fn has_admitted_error(&self, name: &str) -> bool {
        self.agents
            .get(name)
            .is_some_and(|agent| agent.admitted_error)
    }

    pub(crate) fn generation(&self, name: &str) -> Option<u64> {
        self.agents
            .get(name)
            .map(|agent| agent.generation)
            .or_else(|| self.completed.get(name).map(|agent| agent.generation))
    }

    pub(crate) fn owns_generation(&self, name: &str, expected: u64) -> bool {
        self.generation(name) == Some(expected)
    }

    pub(crate) fn owns_active_generation(&self, name: &str, expected: u64) -> bool {
        self.owns_active_lease(&AgentExecutionRef::local(name, expected))
    }

    pub(crate) fn owns_lease(&self, execution: &AgentExecutionRef) -> bool {
        execution.address.is_local()
            && self
                .agents
                .get(&execution.address.agent)
                .is_some_and(|agent| agent.generation == execution.connection_generation)
    }

    pub(crate) fn owns_active_lease(&self, execution: &AgentExecutionRef) -> bool {
        self.owns_lease(execution)
            && self
                .agents
                .get(&execution.address.agent)
                .is_some_and(|agent| !agent.info.lifecycle.is_terminal())
    }
}
