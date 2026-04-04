use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::frontend::traits::EventEmitter;
use loopal_error::{LoopalError, Result};
use loopal_protocol::{AgentEvent, AgentEventPayload};

/// Cloneable event emitter backed by an mpsc sender.
///
/// Wraps each `AgentEventPayload` into a full `AgentEvent` with the
/// configured `agent_name` before sending through the channel.
#[derive(Clone)]
pub struct ChannelEventEmitter {
    tx: mpsc::Sender<AgentEvent>,
    agent_name: Option<String>,
}

impl ChannelEventEmitter {
    pub fn new(tx: mpsc::Sender<AgentEvent>, agent_name: Option<String>) -> Self {
        Self { tx, agent_name }
    }
}

#[async_trait]
impl EventEmitter for ChannelEventEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            event_id: loopal_protocol::event_id::next_event_id(),
            turn_id: loopal_protocol::event_id::current_turn_id(),
            correlation_id: loopal_protocol::event_id::current_correlation_id(),
            payload,
        };
        self.tx
            .send(event)
            .await
            .map_err(|e| LoopalError::Other(format!("event channel closed: {e}")))
    }
}
