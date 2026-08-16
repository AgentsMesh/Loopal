use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;
use tokio::sync::mpsc::Receiver;

use super::hub::{HubHarness, TIMEOUT};
use super::ui::register_ui_client_with_capabilities;

pub struct PermissionApproval {
    pub agent_name: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub workflow: loopal_protocol::WorkflowPermissionCausation,
}

pub struct PermissionClient {
    conn: Arc<Connection<Listening>>,
    rx: Receiver<Incoming>,
}

impl HubHarness {
    pub async fn permission_client(&self, name: &str) -> PermissionClient {
        let (conn, rx) =
            register_ui_client_with_capabilities(&self.hub_addr, &self.hub_token, name, true).await;
        let mut client = PermissionClient { conn, rx };
        client.drain_backlog().await;
        client
    }
}

impl PermissionClient {
    async fn drain_backlog(&mut self) {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(1500), self.rx.recv()).await {
                Ok(Some(Incoming::Notification { method, params }))
                    if method == methods::AGENT_EVENT.name =>
                {
                    if let Ok(event) = serde_json::from_value::<AgentEvent>(params)
                        && matches!(event.payload, AgentEventPayload::AwaitingInput)
                    {
                        return;
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => return,
            }
        }
    }

    pub async fn approve_next(mut self, expected_tool_name: String) -> (Self, PermissionApproval) {
        let approval = tokio::time::timeout(TIMEOUT, async {
            loop {
                let Some(Incoming::Notification { method, params }) = self.rx.recv().await else {
                    panic!("permission UI disconnected before receiving a request");
                };
                if method != methods::AGENT_EVENT.name {
                    continue;
                }
                let Ok(event) = serde_json::from_value::<AgentEvent>(params) else {
                    continue;
                };
                let AgentEventPayload::ToolPermissionRequest {
                    id,
                    name,
                    input,
                    permission_intent,
                } = event.payload
                else {
                    continue;
                };
                let agent_name = event
                    .agent_name
                    .as_ref()
                    .map(|address| address.agent.clone())
                    .expect("workflow permission request must name its worker");
                assert_eq!(name, expected_tool_name);
                let intent = permission_intent.expect("workflow permission intent");
                let workflow = intent
                    .seed()
                    .workflow()
                    .cloned()
                    .expect("workflow permission causation");
                let response = self
                    .conn
                    .send_request(
                        methods::HUB_PERMISSION_RESPONSE.name,
                        json!({
                            "agent_name": agent_name,
                            "tool_call_id": id,
                            "permission_intent_digest": intent.intent_digest(),
                            "allow": true,
                            "remember_session": false,
                        }),
                    )
                    .await
                    .expect("approve workflow permission");
                assert_eq!(
                    response["resolved"], true,
                    "permission response: {response}"
                );
                return PermissionApproval {
                    agent_name,
                    tool_name: name,
                    input,
                    workflow,
                };
            }
        })
        .await
        .expect("workflow permission request timed out");
        (self, approval)
    }
}
