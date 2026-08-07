use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, UiCapability};

use super::cleanup::{InteractionKind, remove_if_current, schedule_timeout};
use super::completion::{TerminalEventSink, complete_detached};
use super::types::{FastPath, PendingPlanApprovalInfo};
use crate::authoritative_events::PreparedAuthoritativeEvent;
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
        reject(agent_conn, agent_ipc_id);
        return;
    };
    let logical_id = params
        .get("request_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let interaction_id = Uuid::new_v4().to_string();
    let event = AgentEvent::named(
        QualifiedAddress::local(agent_name),
        AgentEventPayload::PlanApprovalRequest {
            id: interaction_id.clone(),
            plan_content: content.to_string(),
            plan_path: path.to_string(),
        },
    );
    let key = (agent_name.to_string(), logical_id.clone());
    let (outcome, timeout) = {
        let mut h = hub.lock().await;
        if !h.ui.has_capability(UiCapability::PlanApproval) {
            (FastPath::DenyNoUi, h.pending_interaction_timeout())
        } else if h
            .pending_plan_approvals
            .keys()
            .any(|(agent, _)| agent == agent_name)
        {
            (FastPath::RejectDuplicate, h.pending_interaction_timeout())
        } else {
            h.pending_plan_approvals.insert(
                key,
                PendingPlanApprovalInfo {
                    agent_conn: agent_conn.clone(),
                    agent_ipc_id,
                    agent_name: agent_name.to_string(),
                    interaction_id: interaction_id.clone(),
                    logical_id: logical_id.clone(),
                },
            );
            let event = h.registry.prepare_generation_event(agent_name, event);
            let outcome =
                FastPath::Pending(Box::new(PreparedAuthoritativeEvent::from_hub(&h, event)));
            (outcome, h.pending_interaction_timeout())
        }
    };
    match outcome {
        FastPath::DenyNoUi => {
            warn!(agent = %agent_name, "plan approval UI unavailable; cancelling");
            cancel(agent_conn, agent_ipc_id, "unavailable");
        }
        FastPath::RejectDuplicate => {
            warn!(agent = %agent_name, request_id = logical_id, "concurrent plan approval request rejected");
            cancel(agent_conn, agent_ipc_id, "superseded");
        }
        FastPath::Pending(mut delivery) => {
            let delivery_hub = hub.clone();
            let delivery_conn = agent_conn.clone();
            let delivery_agent = agent_name.to_string();
            let delivery_logical_id = logical_id.clone();
            let delivery_interaction_id = interaction_id.clone();
            let coordinator = tokio::spawn(async move {
                match delivery.deliver().await {
                    Ok(()) => schedule_timeout(
                        &delivery_hub,
                        InteractionKind::PlanApproval,
                        delivery_agent,
                        delivery_logical_id,
                        delivery_interaction_id,
                        timeout,
                    ),
                    Err(error) => {
                        warn!(
                            agent = %delivery_agent,
                            request_id = %delivery_logical_id,
                            %error,
                            "plan approval event admission failed; cancelling request"
                        );
                        if remove_if_current(
                            &delivery_hub,
                            InteractionKind::PlanApproval,
                            &delivery_agent,
                            &delivery_logical_id,
                            &delivery_interaction_id,
                        )
                        .await
                        {
                            cancel(delivery_conn, agent_ipc_id, "unavailable");
                        }
                    }
                }
            });
            if let Err(error) = coordinator.await {
                tracing::error!(agent = %agent_name, %error, "plan approval admission coordinator failed");
                hub.lock().await.shutdown_signal.notify_one();
                if remove_if_current(
                    hub,
                    InteractionKind::PlanApproval,
                    agent_name,
                    &logical_id,
                    &interaction_id,
                )
                .await
                {
                    cancel(agent_conn, agent_ipc_id, "unavailable");
                }
            }
        }
    }
}

pub async fn resolve_plan_approval(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    interaction_id: &str,
    response: serde_json::Value,
) -> bool {
    let (info, terminal_sink) = {
        let mut h = hub.lock().await;
        let key = h
            .pending_plan_approvals
            .iter()
            .find(|((agent, _), info)| agent == agent_name && info.interaction_id == interaction_id)
            .map(|(key, _)| key.clone());
        let info = key.and_then(|key| h.pending_plan_approvals.remove(&key));
        (info, TerminalEventSink::from_hub(&h))
    };
    let Some(info) = info else { return false };
    info!(agent = %info.agent_name, logical_id = %info.logical_id, interaction_id, "plan approval resolved");
    let event = AgentEvent::named(
        QualifiedAddress::local(&info.agent_name),
        AgentEventPayload::PlanApprovalResolved {
            id: info.interaction_id.clone(),
        },
    );
    complete_detached(
        info.agent_conn,
        info.agent_ipc_id,
        response,
        Some((terminal_sink, event)),
    );
    true
}

fn reject(conn: Arc<Connection<Listening>>, id: i64) {
    complete_detached(conn, id, serde_json::json!({"decision": "reject"}), None);
}

fn cancel(conn: Arc<Connection<Listening>>, id: i64, reason: &str) {
    complete_detached(
        conn,
        id,
        serde_json::json!({"decision": "cancelled", "reason": reason}),
        None,
    );
}
