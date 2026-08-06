use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, UiCapability};
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use super::cleanup::{InteractionKind, remove_if_current, schedule_timeout};
use super::completion::complete_detached;
use super::types::{FastPath, PendingPermissionInfo};
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
            let outcome = if h.registry.event_sender().try_send(event).is_err() {
                FastPath::EmitFailed
            } else {
                FastPath::Pending
            };
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
        FastPath::EmitFailed => {
            warn!(agent = %agent_name, tool_call_id, "ToolPermissionRequest dropped (channel full); denying");
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
        FastPath::Pending => schedule_timeout(
            hub,
            InteractionKind::Permission,
            agent_name.to_string(),
            tool_call_id,
            interaction_id,
            timeout,
        ),
    }
}
