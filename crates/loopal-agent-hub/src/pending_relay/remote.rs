use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::AgentEventPayload;
use serde_json::json;
use tokio::sync::Mutex;

use super::PendingQuestionInfo;
use crate::Hub;

pub(super) async fn relay_question_if_remote(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection<Listening>>,
    agent_ipc_id: i64,
    agent_name: &str,
    question_id: &str,
    payload: AgentEventPayload,
) -> bool {
    let route = {
        let mut locked = hub.lock().await;
        if !locked.ui.clients_is_empty() {
            return false;
        }
        let Some(parent) = locked
            .registry
            .agent_info(agent_name)
            .and_then(|info| info.parent.as_ref())
            .filter(|parent| parent.is_remote())
            .cloned()
        else {
            return false;
        };
        let Some(uplink) = locked.uplink.clone() else {
            return false;
        };
        let Some(target_hub) = parent.next_hop().map(String::from) else {
            return false;
        };
        locked.pending_questions.insert(
            (agent_name.to_string(), question_id.to_string()),
            PendingQuestionInfo {
                agent_conn: agent_conn.clone(),
                agent_ipc_id,
                agent_name: agent_name.to_string(),
            },
        );
        (uplink, target_hub)
    };
    let result = route
        .0
        .relay_remote(json!({
            "target_hub": route.1,
            "operation": "question_request",
            "origin_hub": route.0.hub_name(),
            "agent_name": agent_name,
            "payload": payload,
        }))
        .await;
    if result
        .as_ref()
        .ok()
        .and_then(|value| value["emitted"].as_bool())
        == Some(true)
    {
        return true;
    }
    let removed = hub
        .lock()
        .await
        .pending_questions
        .remove(&(agent_name.to_string(), question_id.to_string()));
    if removed.is_some() {
        let _ = agent_conn
            .respond(
                agent_ipc_id,
                json!({"answers": ["(remote UI unavailable)"]}),
            )
            .await;
    }
    true
}
