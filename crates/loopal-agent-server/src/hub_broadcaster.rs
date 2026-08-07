use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use loopal_error::{LoopalError, Result};
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};
use loopal_runtime::frontend::traits::EventEmitter;

use crate::ipc_handlers::SessionRef;
use crate::session_hub::SharedSession;

#[derive(Clone)]
pub(crate) struct HubBroadcaster {
    session: SessionRef,
    agent_name: Option<QualifiedAddress>,
    delivery_tx: mpsc::UnboundedSender<DeliveryRequest>,
}

struct DeliveryRequest {
    event: AgentEvent,
    completion: Option<oneshot::Sender<Result<()>>>,
}

impl HubBroadcaster {
    pub fn new(session: SessionRef, agent_name: Option<QualifiedAddress>) -> Self {
        // `try_emit` is synchronous and is also used from Drop guards. An
        // unbounded admission queue lets those guards enqueue without taking
        // an async lock; the single consumer provides ordering, while each
        // transport write remains bounded by event_delivery's deadline.
        let (delivery_tx, delivery_rx) = mpsc::unbounded_channel();
        tokio::spawn(delivery_loop(session.clone(), delivery_rx));
        Self {
            session,
            agent_name,
            delivery_tx,
        }
    }

    pub async fn replace_session(&self, session: Arc<SharedSession>) {
        *self.session.write().await = session;
    }

    fn build_event(&self, payload: AgentEventPayload) -> AgentEvent {
        AgentEvent::for_agent(self.agent_name.clone(), payload)
    }

    fn build_event_in_turn(&self, payload: AgentEventPayload) -> AgentEvent {
        AgentEvent::for_agent_in_turn(self.agent_name.clone(), payload)
    }

    pub async fn broadcast(&self, payload: AgentEventPayload) -> Result<()> {
        let event = self.build_event(payload);
        self.dispatch(event).await
    }

    pub async fn broadcast_in_turn(&self, payload: AgentEventPayload) -> Result<()> {
        let event = self.build_event_in_turn(payload);
        self.dispatch(event).await
    }

    async fn dispatch(&self, event: AgentEvent) -> Result<()> {
        let (completion_tx, completion_rx) = oneshot::channel();
        self.delivery_tx
            .send(DeliveryRequest {
                event,
                completion: Some(completion_tx),
            })
            .map_err(|_| LoopalError::Ipc("agent event delivery worker stopped".into()))?;
        completion_rx
            .await
            .map_err(|_| LoopalError::Ipc("agent event delivery worker stopped".into()))?
    }

    pub fn try_broadcast(&self, payload: AgentEventPayload) -> bool {
        let event = self.build_event(payload);
        self.delivery_tx
            .send(DeliveryRequest {
                event,
                completion: None,
            })
            .is_ok()
    }
}

async fn delivery_loop(
    session: SessionRef,
    mut delivery_rx: mpsc::UnboundedReceiver<DeliveryRequest>,
) {
    while let Some(request) = delivery_rx.recv().await {
        let outcome = deliver_event(&session, &request.event).await;
        match request.completion {
            Some(completion) => {
                if let Err(outcome) = completion.send(outcome)
                    && let Err(error) = outcome
                {
                    tracing::error!(%error, "agent event delivery failed after caller cancelled");
                }
            }
            None => {
                if let Err(error) = outcome {
                    tracing::error!(%error, "queued best-effort agent event delivery failed");
                }
            }
        }
    }
}

async fn deliver_event(session: &SessionRef, event: &AgentEvent) -> Result<()> {
    let params = serde_json::to_value(event)
        .map_err(|error| LoopalError::Ipc(format!("serialize event: {error}")))?;
    let session = session.read().await.clone();
    crate::event_delivery::deliver(&session, params)
        .await
        .map_err(|error| LoopalError::Ipc(error.to_string()))
}

#[async_trait]
impl EventEmitter for HubBroadcaster {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.broadcast(payload).await
    }
}

#[cfg(test)]
#[path = "hub_broadcaster/tests.rs"]
mod tests;
