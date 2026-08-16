//! ACP adapter: event loop and bootstrap drain.
//!
//! Single source: broadcast `AgentEvent`s from Hub. Permission/question
//! requests now arrive as `ToolPermissionRequest` / `UserQuestionRequest`
//! events; responses go back via `hub/permission_response` /
//! `hub/question_response` through `HubClient`.

use agent_client_protocol_schema::StopReason;
use loopal_protocol::{AgentEvent, AgentEventPayload, ROOT_AGENT_NAME};
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
                    warn!(
                        skipped = n,
                        "event receiver lagged; resyncing root snapshot"
                    );
                    self.replay_loopal_snapshot(session_id).await;
                }
            }
        }
    }

    async fn handle_event(&self, event: &AgentEvent, session_id: &str) -> Option<StopReason> {
        let root_event = is_local_root_event(event);
        match &event.payload {
            AgentEventPayload::AwaitingInput | AgentEventPayload::Finished if root_event => {
                return Some(StopReason::EndTurn);
            }
            AgentEventPayload::ToolPermissionRequest {
                id,
                name,
                input,
                permission_intent,
            } => {
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
                    permission_intent
                        .as_deref()
                        .map(loopal_protocol::PermissionIntent::intent_digest),
                    session_id,
                )
                .await;
                return None;
            }
            AgentEventPayload::UserQuestionRequest { id, questions, .. } => {
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
        if !root_event {
            return None;
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
        // Mode dual-emit: translate_event already sent the standard ACP
        // CurrentModeUpdate (for generic clients). Additionally surface
        // `_loopal/mode` so the Loopal console status bar reads mode from the
        // same loopalSession mirror as thinking/model — AgentsMesh does not
        // consume the ACP session-mode channel.
        if let AgentEventPayload::ModeChanged { mode } = &event.payload {
            let (method, params) = crate::translate::ext::ext_notification(
                session_id,
                "mode",
                serde_json::json!({ "mode": mode }),
            );
            self.acp_out.notify(&method, params).await;
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
                    ) && is_local_root_event(&event) =>
                {
                    return;
                }
                Err(_) => return,
                _ => continue,
            }
        }
    }
}

fn is_local_root_event(event: &AgentEvent) -> bool {
    event
        .agent_name
        .as_ref()
        .is_none_or(|address| address.is_local() && address.agent == ROOT_AGENT_NAME)
}

#[cfg(test)]
#[path = "events/tests.rs"]
mod tests;
