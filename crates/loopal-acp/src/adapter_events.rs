//! ACP adapter: event loop, session creation, bootstrap drain.

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, UserQuestionResponse};
use serde_json::Value;
use tracing::info;

use crate::adapter::AcpAdapter;
use crate::jsonrpc;
use crate::translate::translate_event;
use crate::types::*;

impl AcpAdapter {
    /// Handle session/new: start agent via IPC, drain bootstrap events.
    pub(crate) async fn handle_new_session_inner(&self, id: i64, params: Value) {
        let params: NewSessionParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                self.acp_out
                    .respond_error(id, jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
                return;
            }
        };
        let start_params = serde_json::json!({"cwd": params.cwd.to_string_lossy()});
        match self
            .agent_conn
            .send_request(methods::AGENT_START.name, start_params)
            .await
        {
            Ok(result) => {
                let sid = result["session_id"].as_str().unwrap_or("").to_string();
                *self.session_id.lock().await = Some(sid.clone());
                let acp_result = NewSessionResult { session_id: sid };
                self.acp_out
                    .respond(id, serde_json::to_value(acp_result).unwrap_or_default())
                    .await;
                info!("ACP session created via agent server");
                self.drain_bootstrap_events().await;
            }
            Err(e) => {
                self.acp_out
                    .respond_error(id, jsonrpc::INTERNAL_ERROR, &e)
                    .await;
            }
        }
    }

    /// Run the event loop during a session/prompt: translate agent events
    /// to ACP notifications, handle permissions and questions.
    pub(crate) async fn run_event_loop(&self, session_id: &str) -> StopReason {
        let mut rx = self.agent_rx.lock().await;
        loop {
            let Some(msg) = rx.recv().await else {
                tracing::warn!("agent connection closed during prompt");
                return StopReason::EndTurn;
            };
            match msg {
                Incoming::Notification { method, params } => {
                    if method == methods::AGENT_EVENT.name {
                        if let Some(reason) = self.handle_agent_event(params, session_id).await {
                            return reason;
                        }
                    }
                }
                Incoming::Request { id, method, params } => {
                    if method == methods::AGENT_PERMISSION.name {
                        self.handle_permission_request(id, params, session_id).await;
                    } else if method == methods::AGENT_QUESTION.name {
                        self.handle_question_request(id, params).await;
                    } else {
                        let _ = self
                            .agent_conn
                            .respond_error(
                                id,
                                loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
                                "unexpected request during prompt",
                            )
                            .await;
                    }
                }
            }
        }
    }

    /// Handle an agent/event notification. Returns Some(StopReason) if the
    /// event indicates the prompt is complete.
    async fn handle_agent_event(&self, params: Value, session_id: &str) -> Option<StopReason> {
        let event: AgentEvent = match serde_json::from_value(params) {
            Ok(e) => e,
            Err(_) => return None,
        };

        match &event.payload {
            AgentEventPayload::AwaitingInput => return Some(StopReason::EndTurn),
            AgentEventPayload::MaxTurnsReached { .. } => {
                return Some(StopReason::MaxTurnRequests);
            }
            AgentEventPayload::Finished => return Some(StopReason::EndTurn),
            _ => {}
        }

        if let Some(update_params) = translate_event(&event.payload, session_id) {
            self.acp_out.notify("session/update", update_params).await;
        }
        None
    }

    /// Forward agent/permission request to IDE as session/requestPermission.
    async fn handle_permission_request(&self, request_id: i64, params: Value, session_id: &str) {
        let acp_params = RequestPermissionParams {
            session_id: session_id.to_string(),
            tool_call_id: params["tool_call_id"].as_str().unwrap_or("").into(),
            tool_name: params["tool_name"].as_str().unwrap_or("").into(),
            tool_input: params["tool_input"].clone(),
        };
        let allow = match self
            .acp_out
            .request(
                "session/requestPermission",
                serde_json::to_value(acp_params).unwrap_or_default(),
            )
            .await
        {
            Ok(value) => serde_json::from_value::<RequestPermissionResult>(value)
                .ok()
                .is_some_and(|r| matches!(r.outcome, PermissionOutcome::Allow)),
            Err(_) => false,
        };
        let _ = self
            .agent_conn
            .respond(request_id, serde_json::json!({"allow": allow}))
            .await;
    }

    /// Forward agent/question request to IDE (not yet supported, auto-respond).
    async fn handle_question_request(&self, request_id: i64, _params: Value) {
        let resp = UserQuestionResponse {
            answers: vec!["(not supported in ACP mode)".into()],
        };
        let _ = self
            .agent_conn
            .respond(request_id, serde_json::to_value(resp).unwrap_or_default())
            .await;
    }

    /// Drain bootstrap events (Started, AwaitingInput) after session creation.
    /// Blocks until AwaitingInput or Finished is received.
    pub(crate) async fn drain_bootstrap_events(&self) {
        let mut rx = self.agent_rx.lock().await;
        loop {
            let Some(msg) = rx.recv().await else {
                return;
            };
            if let Incoming::Notification { method, params } = msg {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params) {
                        if matches!(
                            event.payload,
                            AgentEventPayload::AwaitingInput | AgentEventPayload::Finished
                        ) {
                            return; // Agent is ready
                        }
                        continue; // Skip other bootstrap events (Started, etc.)
                    }
                }
            }
        }
    }
}
