use std::time::Duration;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, ControlDisposition, McpServerSnapshot,
    ROOT_AGENT_NAME,
};
use serde_json::json;

use super::hub::{HubHarness, TIMEOUT};

impl HubHarness {
    pub async fn control(&self, command: ControlCommand) -> ControlDisposition {
        let response = self
            .conn
            .send_request(
                methods::HUB_CONTROL.name,
                json!({
                    "target": ROOT_AGENT_NAME,
                    "command": serde_json::to_value(command).unwrap(),
                }),
            )
            .await
            .expect("hub/control");
        ControlDisposition::from_wire_value(response).expect("typed control disposition")
    }

    pub async fn await_mcp_server(&mut self, name: &str) -> McpServerSnapshot {
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            let incoming = tokio::time::timeout(Duration::from_secs(5), self.rx.recv()).await;
            let Ok(Some(Incoming::Notification { method, params })) = incoming else {
                continue;
            };
            if method != methods::AGENT_EVENT.name {
                continue;
            }
            let Ok(event) = serde_json::from_value::<AgentEvent>(params) else {
                continue;
            };
            if let AgentEventPayload::McpStatusReport { servers } = event.payload
                && let Some(server) = servers.into_iter().find(|server| server.name == name)
            {
                return server;
            }
        }
        panic!("timed out waiting for MCP status of {name}")
    }
}
