//! Cloneable event emitter for sub-agent spawning (IPC variant).

use std::sync::Arc;

use async_trait::async_trait;

use loopal_error::{LoopalError, Result};
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_runtime::frontend::traits::EventEmitter;

#[derive(Clone)]
pub(crate) struct IpcEventEmitter {
    pub connection: Arc<Connection>,
    pub agent_name: Option<String>,
}

#[async_trait]
impl EventEmitter for IpcEventEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            event_id: loopal_protocol::event_id::next_event_id(),
            turn_id: loopal_protocol::event_id::current_turn_id(),
            correlation_id: loopal_protocol::event_id::current_correlation_id(),
            payload,
        };
        let params = serde_json::to_value(&event)
            .map_err(|e| LoopalError::Ipc(format!("serialize event: {e}")))?;
        self.connection
            .send_notification(methods::AGENT_EVENT.name, params)
            .await
            .map_err(LoopalError::Ipc)
    }
}
