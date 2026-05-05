use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use loopal_ipc::connection::Connection;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, Question};

use super::types::{FastPath, PendingPermissionInfo, PendingQuestionInfo};
use crate::hub::Hub;

pub async fn handle_agent_permission(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection>,
    agent_ipc_id: i64,
    params: serde_json::Value,
    agent_name: &str,
) {
    let tool_call_id = params
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let tool_name = params
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let tool_input = params.get("tool_input").cloned().unwrap_or_default();

    if tool_call_id.is_empty() {
        warn!(agent = %agent_name, "agent/permission missing tool_call_id, denying");
        let _ = agent_conn
            .respond(agent_ipc_id, serde_json::json!({"allow": false}))
            .await;
        return;
    }

    let event = AgentEvent::named(
        QualifiedAddress::local(agent_name),
        AgentEventPayload::ToolPermissionRequest {
            id: tool_call_id.clone(),
            name: tool_name.to_string(),
            input: tool_input,
        },
    );
    let key = (agent_name.to_string(), tool_call_id.clone());
    let outcome = {
        let mut h = hub.lock().await;
        if h.ui.clients_is_empty() {
            FastPath::DenyNoUi
        } else {
            h.pending_permissions.insert(
                key.clone(),
                PendingPermissionInfo {
                    agent_conn: agent_conn.clone(),
                    agent_ipc_id,
                    agent_name: agent_name.to_string(),
                },
            );
            if h.registry.event_sender().try_send(event).is_err() {
                FastPath::EmitFailed
            } else {
                FastPath::Pending
            }
        }
    };

    match outcome {
        FastPath::DenyNoUi => {
            warn!(agent = %agent_name, "no UI client, denying permission");
            let _ = agent_conn
                .respond(agent_ipc_id, serde_json::json!({"allow": false}))
                .await;
        }
        FastPath::EmitFailed => {
            warn!(agent = %agent_name, tool_call_id, "ToolPermissionRequest dropped (channel full); denying");
            let removed = hub.lock().await.pending_permissions.remove(&key);
            if removed.is_some() {
                let _ = agent_conn
                    .respond(agent_ipc_id, serde_json::json!({"allow": false}))
                    .await;
            }
        }
        FastPath::Pending => {}
    }
}

pub async fn handle_agent_question(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection>,
    agent_ipc_id: i64,
    params: serde_json::Value,
    agent_name: &str,
) {
    let questions: Vec<Question> =
        match serde_json::from_value(params.get("questions").cloned().unwrap_or_default()) {
            Ok(q) => q,
            Err(e) => {
                warn!(agent = %agent_name, error = %e, "agent/question malformed, denying");
                let _ = agent_conn
                    .respond(
                        agent_ipc_id,
                        serde_json::json!({"answers": ["(parse error)"]}),
                    )
                    .await;
                return;
            }
        };
    let question_id = Uuid::new_v4().to_string();
    let event = AgentEvent::named(
        QualifiedAddress::local(agent_name),
        AgentEventPayload::UserQuestionRequest {
            id: question_id.clone(),
            questions,
        },
    );
    let key = (agent_name.to_string(), question_id.clone());
    let outcome = {
        let mut h = hub.lock().await;
        if h.ui.clients_is_empty() {
            FastPath::DenyNoUi
        } else {
            h.pending_questions.insert(
                key.clone(),
                PendingQuestionInfo {
                    agent_conn: agent_conn.clone(),
                    agent_ipc_id,
                    agent_name: agent_name.to_string(),
                },
            );
            if h.registry.event_sender().try_send(event).is_err() {
                FastPath::EmitFailed
            } else {
                FastPath::Pending
            }
        }
    };

    match outcome {
        FastPath::DenyNoUi => {
            warn!(agent = %agent_name, "no UI client, auto-answering");
            let _ = agent_conn
                .respond(
                    agent_ipc_id,
                    serde_json::json!({"answers": ["(no UI client)"]}),
                )
                .await;
        }
        FastPath::EmitFailed => {
            warn!(agent = %agent_name, question_id, "UserQuestionRequest dropped (channel full); answering with error");
            let removed = hub.lock().await.pending_questions.remove(&key);
            if removed.is_some() {
                let _ = agent_conn
                    .respond(
                        agent_ipc_id,
                        serde_json::json!({"answers": ["(event dropped)"]}),
                    )
                    .await;
            }
        }
        FastPath::Pending => {}
    }
}
