use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};

use super::types::PendingPlanApprovalInfo;
use crate::hub::Hub;

pub async fn handle_agent_plan_approval(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection<Listening>>,
    agent_ipc_id: i64,
    params: serde_json::Value,
    agent_name: &str,
) {
    let content = params.get("plan_content").and_then(|v| v.as_str());
    let path = params.get("plan_path").and_then(|v| v.as_str());
    let (Some(content), Some(path)) = (content, path) else {
        reject(&agent_conn, agent_ipc_id).await;
        return;
    };
    let id = Uuid::new_v4().to_string();
    let event = AgentEvent::named(
        QualifiedAddress::local(agent_name),
        AgentEventPayload::PlanApprovalRequest {
            id: id.clone(),
            plan_content: content.to_string(),
            plan_path: path.to_string(),
        },
    );
    let key = (agent_name.to_string(), id.clone());
    let pending = {
        let mut h = hub.lock().await;
        if h.ui.clients_is_empty() {
            false
        } else {
            h.pending_plan_approvals.insert(
                key.clone(),
                PendingPlanApprovalInfo {
                    agent_conn: agent_conn.clone(),
                    agent_ipc_id,
                    agent_name: agent_name.to_string(),
                },
            );
            h.registry.event_sender().try_send(event).is_ok()
        }
    };
    if !pending {
        hub.lock().await.pending_plan_approvals.remove(&key);
        warn!(agent = %agent_name, "plan approval unavailable; rejecting");
        reject(&agent_conn, agent_ipc_id).await;
    }
}

pub async fn resolve_plan_approval(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    request_id: &str,
    response: serde_json::Value,
) -> bool {
    let key = (agent_name.to_string(), request_id.to_string());
    let info = hub.lock().await.pending_plan_approvals.remove(&key);
    let Some(info) = info else { return false };
    info!(agent = %info.agent_name, request_id, "plan approval resolved");
    let _ = info.agent_conn.respond(info.agent_ipc_id, response).await;
    let event = AgentEvent::named(
        QualifiedAddress::local(&info.agent_name),
        AgentEventPayload::PlanApprovalResolved {
            id: request_id.to_string(),
        },
    );
    if hub
        .lock()
        .await
        .registry
        .event_sender()
        .try_send(event)
        .is_err()
    {
        warn!(agent = %info.agent_name, "PlanApprovalResolved event dropped");
    }
    true
}

async fn reject(conn: &Connection<Listening>, id: i64) {
    let _ = conn
        .respond(id, serde_json::json!({"decision": "reject"}))
        .await;
}
