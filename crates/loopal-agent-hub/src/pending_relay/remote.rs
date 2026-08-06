use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEventPayload, UiCapability, UserQuestionResponse};
use serde_json::json;
use tokio::sync::Mutex;

use super::PendingQuestionInfo;
use super::cleanup::{InteractionKind, schedule_timeout};
use super::completion::complete_detached;
use super::types::InteractionAudience;
use crate::Hub;

pub(super) async fn relay_question_if_remote(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection<Listening>>,
    agent_ipc_id: i64,
    agent_name: &str,
    question_id: &str,
    interaction_id: &str,
    payload: AgentEventPayload,
) -> bool {
    let route = {
        let mut locked = hub.lock().await;
        if locked.ui.has_capability(UiCapability::Question) {
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
        let key = (agent_name.to_string(), question_id.to_string());
        if locked
            .pending_questions
            .keys()
            .any(|(agent, _)| agent == agent_name)
        {
            drop(locked);
            let response = serde_json::to_value(UserQuestionResponse::cancelled(question_id))
                .unwrap_or(serde_json::Value::Null);
            complete_detached(agent_conn, agent_ipc_id, response, None);
            return true;
        }
        locked.pending_questions.insert(
            key,
            PendingQuestionInfo {
                agent_conn: agent_conn.clone(),
                agent_ipc_id,
                agent_name: agent_name.to_string(),
                interaction_id: interaction_id.to_string(),
                logical_id: question_id.to_string(),
                audience: InteractionAudience::RemoteUi {
                    target_hub: target_hub.clone(),
                    uplink: uplink.clone(),
                },
            },
        );
        (uplink, target_hub, locked.pending_interaction_timeout())
    };
    let (uplink, target_hub, timeout) = route;
    let origin_hub = uplink.hub_name().to_string();
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    let result = uplink
        .relay_remote(json!({
            "target_hub": target_hub,
            "operation": "question_request",
            "origin_hub": origin_hub,
            "agent_name": agent_name,
            "timeout_ms": timeout_ms,
            "payload": payload,
        }))
        .await;
    if result
        .as_ref()
        .ok()
        .and_then(|value| value["emitted"].as_bool())
        == Some(true)
    {
        schedule_timeout(
            hub,
            InteractionKind::Question,
            agent_name.to_string(),
            question_id.to_string(),
            interaction_id.to_string(),
            timeout,
        );
        return true;
    }
    let removed = {
        let mut h = hub.lock().await;
        let key = (agent_name.to_string(), question_id.to_string());
        if h.pending_questions
            .get(&key)
            .is_some_and(|info| info.interaction_id == interaction_id)
        {
            h.pending_questions.remove(&key)
        } else {
            None
        }
    };
    if removed.is_some() {
        let response = serde_json::to_value(UserQuestionResponse::unsupported(
            question_id,
            "remote UI unavailable",
        ))
        .unwrap_or(serde_json::Value::Null);
        complete_detached(agent_conn, agent_ipc_id, response, None);
    }
    true
}
