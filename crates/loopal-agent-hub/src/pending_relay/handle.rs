use std::sync::Arc;

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::UiCapability;
use tokio::sync::Mutex;
use tracing::warn;
use uuid::Uuid;

use super::completion::complete_detached;
use super::permission_delivery::{PermissionDelivery, coordinate, deny};
use super::permission_request::PermissionRequest;
use super::types::{PendingPermissionInfo, PermissionFastPath};
use crate::authoritative_events::PreparedAuthoritativeEvent;
use crate::hub::Hub;
use crate::types::AgentExecutionRef;

#[cfg(test)]
#[path = "handle_tests.rs"]
mod tests;

pub(crate) async fn handle_agent_permission(
    hub: &Arc<Mutex<Hub>>,
    agent_conn: Arc<Connection<Listening>>,
    agent_ipc_id: i64,
    params: serde_json::Value,
    agent_name: &str,
    execution: AgentExecutionRef,
) {
    let request = match PermissionRequest::parse(params) {
        Ok(request) => request,
        Err(error) => {
            warn!(agent = %agent_name, %error, "invalid Agent permission request");
            deny(agent_conn, agent_ipc_id);
            return;
        }
    };
    let tool_call_id = request.logical_id.clone();
    let interaction_id = Uuid::new_v4().to_string();
    let key = (agent_name.to_string(), tool_call_id.clone());
    let (outcome, timeout) = {
        let mut h = hub.lock().await;
        let timeout = h.pending_interaction_timeout();
        let active = h.registry.owns_active_lease(&execution)
            && h.registry
                .exact_connection(&execution)
                .is_some_and(|active| Arc::ptr_eq(&active, &agent_conn));
        let workflow_authorized = h.registry.runtime_facts(&execution).is_some_and(|facts| {
            request.matches_workflow_authority(facts.workflow_permission_causation.as_ref())
        });
        if !active {
            (PermissionFastPath::DenyStale, timeout)
        } else if request.is_legacy() || !workflow_authorized {
            (PermissionFastPath::DenyInvalid, timeout)
        } else if let Some(fast_path) =
            super::permission_fast_path::authorize(&h, &request, &execution)
        {
            match fast_path {
                Ok(outcome) => (outcome, timeout),
                Err(error) => {
                    warn!(agent = %agent_name, %error, "permission receipt issuance failed");
                    (PermissionFastPath::DenyInvalid, timeout)
                }
            }
        } else {
            let ui = h.ui.capability_snapshot();
            if !ui.capabilities.supports(UiCapability::Permission) {
                (PermissionFastPath::DenyNoUi, timeout)
            } else if h
                .pending_permissions
                .keys()
                .any(|(agent, _)| agent == agent_name)
            {
                (PermissionFastPath::RejectDuplicate, timeout)
            } else {
                match request.bind(
                    execution.connection_generation,
                    ui.generation,
                    &interaction_id,
                ) {
                    Err(error) => {
                        warn!(agent = %agent_name, %error, "permission intent binding failed");
                        (PermissionFastPath::DenyInvalid, timeout)
                    }
                    Ok(None) => (PermissionFastPath::DenyInvalid, timeout),
                    Ok(Some(intent)) => {
                        let event =
                            request.event(agent_name, interaction_id.clone(), Some(intent.clone()));
                        h.pending_permissions.insert(
                            key,
                            PendingPermissionInfo {
                                execution: execution.clone(),
                                agent_conn: agent_conn.clone(),
                                agent_ipc_id,
                                agent_name: agent_name.to_string(),
                                interaction_id: interaction_id.clone(),
                                logical_id: tool_call_id.clone(),
                                tool_name: request.tool_name.clone(),
                                permission_intent: intent,
                            },
                        );
                        let event = h
                            .registry
                            .prepare_execution_event(&execution, event)
                            .expect("active permission lease changed while Hub was locked");
                        (
                            PermissionFastPath::Pending(Box::new(
                                PreparedAuthoritativeEvent::from_hub(&h, event),
                            )),
                            timeout,
                        )
                    }
                }
            }
        }
    };

    match outcome {
        PermissionFastPath::Authorize {
            audit: request,
            intent,
            source,
        } => {
            if let Err(error) = super::permission_audit::record_for_execution(
                hub,
                &execution,
                &agent_conn,
                &request,
            )
            .await
            {
                warn!(agent = %agent_name, %error, ?source, "permission fast-path audit failed");
                deny(agent_conn, agent_ipc_id);
            } else {
                let receipt = {
                    let mut h = hub.lock().await;
                    let current = h.registry.owns_active_lease(&execution)
                        && h.registry
                            .exact_connection(&execution)
                            .is_some_and(|active| Arc::ptr_eq(&active, &agent_conn))
                        && super::permission_fast_path::authority_is_current(
                            &h, &intent, &execution, source,
                        );
                    current
                        .then(|| h.permission_receipts.issue(&intent, &execution, false))
                        .transpose()
                };
                match receipt {
                    Ok(Some(receipt)) => complete_detached(
                        agent_conn,
                        agent_ipc_id,
                        serde_json::json!({"allow": true, "permission_receipt": receipt}),
                        None,
                    ),
                    Ok(None) => {
                        warn!(agent = %agent_name, "stale permission lease after audit");
                        deny(agent_conn, agent_ipc_id);
                    }
                    Err(error) => {
                        warn!(agent = %agent_name, %error, "permission receipt issuance failed");
                        deny(agent_conn, agent_ipc_id);
                    }
                }
            }
        }
        PermissionFastPath::DenyInvalid => {
            warn!(agent = %agent_name, "unbound permission request denied");
            deny(agent_conn, agent_ipc_id);
        }
        PermissionFastPath::DenyStale => {
            warn!(agent = %agent_name, "stale Agent permission lease denied");
            deny(agent_conn, agent_ipc_id);
        }
        PermissionFastPath::DenyNoUi => {
            warn!(agent = %agent_name, "no permission-capable UI");
            deny(agent_conn, agent_ipc_id);
        }
        PermissionFastPath::RejectDuplicate => {
            warn!(agent = %agent_name, tool_call_id, "concurrent permission request rejected");
            deny(agent_conn, agent_ipc_id);
        }
        PermissionFastPath::Pending(event) => {
            coordinate(
                hub,
                PermissionDelivery {
                    event,
                    agent_conn,
                    agent_ipc_id,
                    agent_name: agent_name.to_string(),
                    tool_call_id,
                    interaction_id,
                    timeout,
                },
            )
            .await;
        }
    }
}
