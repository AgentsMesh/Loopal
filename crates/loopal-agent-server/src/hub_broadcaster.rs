use std::sync::Arc;

use async_trait::async_trait;

use loopal_error::{LoopalError, Result};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};
use loopal_runtime::frontend::traits::EventEmitter;

use crate::ipc_handlers::SessionRef;
use crate::session_hub::SharedSession;

#[derive(Clone)]
pub(crate) struct HubBroadcaster {
    session: SessionRef,
    agent_name: Option<QualifiedAddress>,
}

impl HubBroadcaster {
    pub fn new(session: SessionRef, agent_name: Option<QualifiedAddress>) -> Self {
        Self {
            session,
            agent_name,
        }
    }

    pub async fn replace_session(&self, session: Arc<SharedSession>) {
        *self.session.write().await = session;
    }

    fn build_event(&self, payload: AgentEventPayload) -> AgentEvent {
        AgentEvent::for_agent(self.agent_name.clone(), payload)
    }

    pub async fn broadcast(&self, payload: AgentEventPayload) -> Result<()> {
        let event = self.build_event(payload);
        let params = serde_json::to_value(&event)
            .map_err(|e| LoopalError::Ipc(format!("serialize event: {e}")))?;
        let session = self.session.read().await.clone();
        let conns = session.all_connections().await;
        let mut dead = Vec::new();
        for (i, conn) in conns.iter().enumerate() {
            if conn
                .send_notification(methods::AGENT_EVENT.name, params.clone())
                .await
                .is_err()
            {
                dead.push(i);
            }
        }
        if !dead.is_empty() {
            session.remove_dead_connections(&dead).await;
        }
        Ok(())
    }

    pub fn try_broadcast(&self, payload: AgentEventPayload) -> bool {
        let event = self.build_event(payload);
        let Ok(params) = serde_json::to_value(&event) else {
            return false;
        };
        let session = match self.session.try_read() {
            Ok(guard) => guard.clone(),
            Err(_) => return false,
        };
        tokio::spawn(async move {
            for conn in session.all_connections().await {
                let _ = conn
                    .send_notification(methods::AGENT_EVENT.name, params.clone())
                    .await;
            }
        });
        true
    }
}

#[async_trait]
impl EventEmitter for HubBroadcaster {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.broadcast(payload).await
    }
}
