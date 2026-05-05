//! ACP adapter: event loop and bootstrap drain.
//!
//! Single source: broadcast `AgentEvent`s from Hub. Permission/question
//! requests now arrive as `ToolPermissionRequest` / `UserQuestionRequest`
//! events; responses go back via `hub/permission_response` /
//! `hub/question_response` through `HubClient`.

use agent_client_protocol_schema::StopReason;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use tracing::warn;

use crate::adapter::AcpAdapter;
use crate::translate::{AcpNotification, translate_event};

impl AcpAdapter {
    pub(crate) async fn run_event_loop(&self, session_id: &str) -> StopReason {
        let mut event_rx = self.event_rx.lock().await;
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    if let Some(r) = self.handle_event(&event, session_id).await {
                        return r;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    warn!("event broadcast closed");
                    return StopReason::EndTurn;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(skipped = n, "event receiver lagged");
                }
            }
        }
    }

    async fn handle_event(&self, event: &AgentEvent, session_id: &str) -> Option<StopReason> {
        match &event.payload {
            AgentEventPayload::AwaitingInput => return Some(StopReason::EndTurn),
            AgentEventPayload::Finished => return Some(StopReason::EndTurn),
            AgentEventPayload::ToolPermissionRequest { id, name, input } => {
                let agent_name = event
                    .agent_name
                    .as_ref()
                    .map(|q| q.agent.clone())
                    .unwrap_or_else(|| "main".to_string());
                self.handle_permission_request(
                    agent_name,
                    id.clone(),
                    name.clone(),
                    input.clone(),
                    session_id,
                )
                .await;
                return None;
            }
            AgentEventPayload::UserQuestionRequest { id, questions } => {
                let agent_name = event
                    .agent_name
                    .as_ref()
                    .map(|q| q.agent.clone())
                    .unwrap_or_else(|| "main".to_string());
                self.handle_question_request(agent_name, id.clone(), questions.clone())
                    .await;
                return None;
            }
            _ => {}
        }
        if let Some(notif) = translate_event(&event.payload, session_id) {
            match notif {
                AcpNotification::SessionUpdate(params) => {
                    self.acp_out.notify("session/update", params).await;
                }
                AcpNotification::Extension { method, params } => {
                    self.acp_out.notify(&method, params).await;
                }
            }
        }
        None
    }

    pub(crate) async fn drain_bootstrap_events(&self) {
        let mut rx = self.event_rx.lock().await;
        loop {
            match rx.recv().await {
                Ok(event)
                    if matches!(
                        event.payload,
                        AgentEventPayload::AwaitingInput | AgentEventPayload::Finished
                    ) =>
                {
                    return;
                }
                Err(_) => return,
                _ => continue,
            }
        }
    }
}
