use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, UiCapability};
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use super::cleanup::{InteractionKind, remove_if_current, schedule_timeout};
use super::completion::complete_detached;
use super::types::{FastPath, PendingPermissionInfo};
use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;

pub async fn handle_agent_permission(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection<Listening>>,
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
        .unwrap_or("")
        .to_string();
    let tool_input = params.get("tool_input").cloned().unwrap_or_default();

    if tool_call_id.is_empty() {
        warn!(agent = %agent_name, "agent/permission missing tool_call_id, denying");
        complete_detached(
            agent_conn,
            agent_ipc_id,
            serde_json::json!({"allow": false}),
            None,
        );
        return;
    }
    let grant_key = (agent_name.to_string(), tool_name.clone());
    if hub
        .lock()
        .await
        .session_permission_grants
        .contains(&grant_key)
    {
        complete_detached(
            agent_conn,
            agent_ipc_id,
            serde_json::json!({"allow": true}),
            None,
        );
        return;
    }

    let interaction_id = Uuid::new_v4().to_string();
    let event = AgentEvent::named(
        QualifiedAddress::local(agent_name),
        AgentEventPayload::ToolPermissionRequest {
            id: interaction_id.clone(),
            name: tool_name.clone(),
            input: tool_input,
        },
    );
    let key = (agent_name.to_string(), tool_call_id.clone());
    let (outcome, timeout) = {
        let mut h = hub.lock().await;
        if !h.ui.has_capability(UiCapability::Permission) {
            (FastPath::DenyNoUi, h.pending_interaction_timeout())
        } else if h
            .pending_permissions
            .keys()
            .any(|(agent, _)| agent == agent_name)
        {
            (FastPath::RejectDuplicate, h.pending_interaction_timeout())
        } else {
            h.pending_permissions.insert(
                key,
                PendingPermissionInfo {
                    agent_conn: agent_conn.clone(),
                    agent_ipc_id,
                    agent_name: agent_name.to_string(),
                    interaction_id: interaction_id.clone(),
                    logical_id: tool_call_id.clone(),
                    tool_name,
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
            warn!(agent = %agent_name, "no permission-capable UI, denying permission");
            complete_detached(
                agent_conn,
                agent_ipc_id,
                serde_json::json!({"allow": false}),
                None,
            );
        }
        FastPath::RejectDuplicate => {
            warn!(agent = %agent_name, tool_call_id, "concurrent permission request rejected");
            complete_detached(
                agent_conn,
                agent_ipc_id,
                serde_json::json!({"allow": false}),
                None,
            );
        }
        FastPath::Pending(mut delivery) => {
            // The coordinator owns both event admission and timeout/cleanup.
            // If the agent IO owner is cancelled while backpressured, dropping
            // the JoinHandle detaches this task instead of stranding pending
            // state without an observable request.
            let delivery_hub = hub.clone();
            let delivery_conn = agent_conn.clone();
            let delivery_agent = agent_name.to_string();
            let delivery_logical_id = tool_call_id.clone();
            let delivery_interaction_id = interaction_id.clone();
            let coordinator = tokio::spawn(async move {
                match delivery.deliver().await {
                    Ok(()) => schedule_timeout(
                        &delivery_hub,
                        InteractionKind::Permission,
                        delivery_agent,
                        delivery_logical_id,
                        delivery_interaction_id,
                        timeout,
                    ),
                    Err(error) => {
                        warn!(
                            agent = %delivery_agent,
                            tool_call_id = %delivery_logical_id,
                            %error,
                            "permission event admission failed; denying request"
                        );
                        if remove_if_current(
                            &delivery_hub,
                            InteractionKind::Permission,
                            &delivery_agent,
                            &delivery_logical_id,
                            &delivery_interaction_id,
                        )
                        .await
                        {
                            complete_detached(
                                delivery_conn,
                                agent_ipc_id,
                                serde_json::json!({"allow": false}),
                                None,
                            );
                        }
                    }
                }
            });
            if let Err(error) = coordinator.await {
                tracing::error!(agent = %agent_name, %error, "permission admission coordinator failed");
                hub.lock().await.shutdown_signal.notify_one();
                if remove_if_current(
                    hub,
                    InteractionKind::Permission,
                    agent_name,
                    &tool_call_id,
                    &interaction_id,
                )
                .await
                {
                    complete_detached(
                        agent_conn,
                        agent_ipc_id,
                        serde_json::json!({"allow": false}),
                        None,
                    );
                }
            }
        }
    }
}
