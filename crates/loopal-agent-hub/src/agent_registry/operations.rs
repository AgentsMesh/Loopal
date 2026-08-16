use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentCompletion, InterruptSignal};

use super::{AgentRegistry, PendingCompletionDelivery};
use crate::types::{AgentConnectionState, AgentExecutionRef};

pub(crate) enum PreparedInterrupt {
    Local {
        signal: InterruptSignal,
        generation_tx: Arc<tokio::sync::watch::Sender<u64>>,
    },
    Connected(Arc<Connection<Listening>>),
    Shadow,
}

impl PreparedInterrupt {
    pub(crate) async fn execute(self, address: &loopal_protocol::QualifiedAddress) -> bool {
        match self {
            Self::Local {
                signal,
                generation_tx,
            } => {
                signal.signal();
                generation_tx.send_modify(|value| *value = value.wrapping_add(1));
                true
            }
            Self::Connected(connection) => connection
                .send_notification(methods::AGENT_INTERRUPT.name, serde_json::json!({}))
                .await
                .is_ok(),
            Self::Shadow => {
                tracing::debug!(agent = %address, "skipping interrupt for shadow entry");
                false
            }
        }
    }
}

impl AgentRegistry {
    pub(crate) fn current_execution(&self, name: &str) -> Option<AgentExecutionRef> {
        self.agents
            .get(name)
            .map(|agent| AgentExecutionRef::local(name, agent.generation))
    }

    pub(crate) fn exact_connection(
        &self,
        execution: &AgentExecutionRef,
    ) -> Option<Arc<Connection<Listening>>> {
        if !self.owns_lease(execution) {
            return None;
        }
        self.agents
            .get(&execution.address.agent)
            .and_then(|agent| agent.state.connection())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn emit_agent_completion_exact(
        &mut self,
        execution: &AgentExecutionRef,
        completion: AgentCompletion,
    ) -> Option<PendingCompletionDelivery> {
        if !self.owns_lease(execution) {
            return None;
        }
        Some(self.emit_agent_completion(&execution.address.agent, completion))
    }

    pub async fn interrupt(&self, name: &str) {
        let Some(execution) = self.current_execution(name) else {
            return;
        };
        if let Some(operation) = self.interrupt_exact(&execution) {
            operation.execute(&execution.address).await;
        }
    }

    pub(crate) fn interrupt_exact(
        &self,
        execution: &AgentExecutionRef,
    ) -> Option<PreparedInterrupt> {
        if !self.owns_lease(execution) {
            return None;
        }
        match &self.agents.get(&execution.address.agent)?.state {
            AgentConnectionState::Local(channels) => Some(PreparedInterrupt::Local {
                signal: channels.interrupt.clone(),
                generation_tx: channels.interrupt_tx.clone(),
            }),
            AgentConnectionState::Connected(connection) => {
                Some(PreparedInterrupt::Connected(connection.clone()))
            }
            AgentConnectionState::Shadow => Some(PreparedInterrupt::Shadow),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn prepare_shutdown_exact(
        &self,
        execution: &AgentExecutionRef,
    ) -> Option<Arc<Connection<Listening>>> {
        self.exact_connection(execution)
    }
}
