use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::frontend::traits::EventEmitter;
use loopal_error::{LoopalError, Result};
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};

#[derive(Clone)]
pub(super) struct ChannelEventEmitter {
    tx: mpsc::Sender<AgentEvent>,
    agent_name: Option<QualifiedAddress>,
}

impl ChannelEventEmitter {
    pub(super) fn new(tx: mpsc::Sender<AgentEvent>, agent_name: Option<QualifiedAddress>) -> Self {
        Self { tx, agent_name }
    }
}

#[async_trait]
impl EventEmitter for ChannelEventEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent::for_agent(self.agent_name.clone(), payload);
        self.tx
            .send(event)
            .await
            .map_err(|e| LoopalError::Other(format!("event channel closed: {e}")))
    }
}
