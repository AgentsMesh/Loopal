//! Cloneable EventEmitter for HubFrontend — used by sub-agent spawning.

use std::sync::Arc;

use async_trait::async_trait;

use loopal_error::{LoopalError, Result};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_runtime::frontend::traits::EventEmitter;

use crate::session_hub::SharedSession;

#[derive(Clone)]
pub(crate) struct HubEventEmitter {
    pub(crate) session: Option<Arc<SharedSession>>,
    pub(crate) agent_name: Option<String>,
}

#[async_trait]
impl EventEmitter for HubEventEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let Some(ref session) = self.session else {
            return Ok(());
        };
        let event = AgentEvent {
            agent_name: self.agent_name.clone(),
            payload,
        };
        let params = serde_json::to_value(&event)
            .map_err(|e| LoopalError::Ipc(format!("serialize event: {e}")))?;
        for conn in session.all_connections().await {
            let _ = conn
                .send_notification(methods::AGENT_EVENT.name, params.clone())
                .await;
        }
        Ok(())
    }
}
