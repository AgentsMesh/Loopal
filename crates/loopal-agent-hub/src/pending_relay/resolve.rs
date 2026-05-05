use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};

use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};

use crate::hub::Hub;

/// Take a pending permission and respond to the agent. Emits Resolved.
/// Returns false if the pending was already removed (race / cleanup).
pub async fn resolve_permission(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    tool_call_id: &str,
    allow: bool,
) -> bool {
    let key = (agent_name.to_string(), tool_call_id.to_string());
    let info = {
        let mut h = hub.lock().await;
        h.pending_permissions.remove(&key)
    };
    let Some(info) = info else {
        return false;
    };
    info!(agent = %info.agent_name, tool_call_id, allow, "permission resolved");
    let _ = info
        .agent_conn
        .respond(info.agent_ipc_id, serde_json::json!({"allow": allow}))
        .await;
    let resolved = AgentEvent::named(
        QualifiedAddress::local(&info.agent_name),
        AgentEventPayload::ToolPermissionResolved {
            id: tool_call_id.to_string(),
        },
    );
    let h = hub.lock().await;
    if h.registry.event_sender().try_send(resolved).is_err() {
        warn!(agent = %info.agent_name, "ToolPermissionResolved event dropped");
    }
    true
}

pub async fn resolve_question(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    question_id: &str,
    answers: Vec<String>,
) -> bool {
    let key = (agent_name.to_string(), question_id.to_string());
    let info = {
        let mut h = hub.lock().await;
        h.pending_questions.remove(&key)
    };
    let Some(info) = info else {
        return false;
    };
    info!(agent = %info.agent_name, question_id, "question resolved");
    let resp = serde_json::json!({"answers": answers});
    let _ = info.agent_conn.respond(info.agent_ipc_id, resp).await;
    let resolved = AgentEvent::named(
        QualifiedAddress::local(&info.agent_name),
        AgentEventPayload::UserQuestionResolved {
            id: question_id.to_string(),
        },
    );
    let h = hub.lock().await;
    if h.registry.event_sender().try_send(resolved).is_err() {
        warn!(agent = %info.agent_name, "UserQuestionResolved event dropped");
    }
    true
}
