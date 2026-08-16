use std::io::Write;
use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{
    AgentCompletion, WorkflowAttemptCapability, WorkflowPermissionCausation,
    WorkflowWorkerHandshakeRequest, WorkflowWorkerHandshakeResponse,
};
use serde_json::json;

const LATE_COMPLETION_CANARY: &str = "late-success-must-not-win";

#[tokio::main]
async fn main() {
    let transport: Arc<dyn Transport> = Arc::new(StdioTransport::from_std());
    let (connection, mut incoming) = Connection::new(transport).into_listening();

    while let Some(message) = incoming.recv().await {
        match message {
            Incoming::Request { id, method, .. } if method == methods::INITIALIZE.name => {
                append_trace("initialize");
                connection
                    .respond(id, json!({"protocol_version": 1}))
                    .await
                    .expect("respond initialize");
            }
            Incoming::Request { id, method, params } if method == methods::AGENT_START.name => {
                let handshake = WorkflowWorkerHandshakeRequest {
                    causation: serde_json::from_value::<WorkflowPermissionCausation>(
                        params["workflow_permission_causation"].clone(),
                    )
                    .expect("workflow permission causation"),
                    capability: serde_json::from_value::<WorkflowAttemptCapability>(
                        params["workflow_attempt_capability"].clone(),
                    )
                    .expect("workflow attempt capability"),
                };
                let response = connection
                    .send_request(
                        methods::HUB_WORKFLOW_WORKER_HANDSHAKE.name,
                        serde_json::to_value(handshake).unwrap(),
                    )
                    .await
                    .expect("workflow worker handshake");
                serde_json::from_value::<WorkflowWorkerHandshakeResponse>(response)
                    .expect("typed workflow worker handshake response");
                append_trace("handshake");
                connection
                    .respond(id, json!({"session_id": params["session_id"]}))
                    .await
                    .expect("respond agent/start");
                append_trace("started");
            }
            Incoming::Notification { method, .. } if method == methods::AGENT_INTERRUPT.name => {
                append_trace("interrupt");
                append_trace("late_completion");
                connection
                    .send_notification(
                        methods::AGENT_COMPLETED.name,
                        serde_json::to_value(AgentCompletion::goal(Some(
                            LATE_COMPLETION_CANARY.into(),
                        )))
                        .unwrap(),
                    )
                    .await
                    .expect("send completion after interrupt");
            }
            Incoming::Request { id, method, .. } if method == methods::AGENT_SHUTDOWN.name => {
                append_trace("shutdown");
                connection
                    .respond(id, json!({}))
                    .await
                    .expect("respond shutdown");
                return;
            }
            _ => {}
        }
    }
}

fn append_trace(line: &str) {
    let path = std::env::var("LOOPAL_E2E_WORKER_TRACE").expect("worker trace path");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open worker trace");
    writeln!(file, "{line}").expect("write worker trace");
    file.sync_all().expect("sync worker trace");
}
