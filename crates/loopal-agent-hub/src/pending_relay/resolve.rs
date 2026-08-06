use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::info;

use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress, ResolveSource};

use super::completion::{TerminalEventSink, complete_detached};
use super::types::InteractionAudience;
use crate::HubUplink;
use crate::hub::Hub;

/// Take a pending permission and respond to the agent. Emits Resolved.
/// Returns false if the pending was already removed (race / cleanup).
pub async fn resolve_permission(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    interaction_id: &str,
    allow: bool,
    remember_session: bool,
) -> bool {
    let (info, terminal_sink) = {
        let mut h = hub.lock().await;
        let key = h
            .pending_permissions
            .iter()
            .find(|((agent, _), info)| agent == agent_name && info.interaction_id == interaction_id)
            .map(|(key, _)| key.clone());
        let info = key.and_then(|key| h.pending_permissions.remove(&key));
        if allow
            && remember_session
            && let Some(info) = info.as_ref()
        {
            h.session_permission_grants
                .insert((info.agent_name.clone(), info.tool_name.clone()));
        }
        (info, TerminalEventSink::from_hub(&h))
    };
    let Some(info) = info else {
        return false;
    };
    info!(agent = %info.agent_name, logical_id = %info.logical_id, interaction_id, allow, "permission resolved");
    let resolved = AgentEvent::named(
        QualifiedAddress::local(&info.agent_name),
        AgentEventPayload::ToolPermissionResolved {
            id: info.interaction_id.clone(),
        },
    );
    complete_detached(
        info.agent_conn,
        info.agent_ipc_id,
        serde_json::json!({"allow": allow}),
        Some((terminal_sink, resolved)),
    );
    true
}

pub async fn resolve_question(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    interaction_id: &str,
    response: loopal_protocol::UserQuestionResponse,
) -> Result<bool, String> {
    resolve_question_for(hub, agent_name, interaction_id, response, None).await
}

pub(crate) async fn resolve_remote_question(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    interaction_id: &str,
    response: loopal_protocol::UserQuestionResponse,
    uplink: &Arc<HubUplink>,
) -> Result<bool, String> {
    resolve_question_for(hub, agent_name, interaction_id, response, Some(uplink)).await
}

async fn resolve_question_for(
    hub: &Arc<Mutex<Hub>>,
    agent_name: &str,
    interaction_id: &str,
    response: loopal_protocol::UserQuestionResponse,
    remote_uplink: Option<&Arc<HubUplink>>,
) -> Result<bool, String> {
    if response.question_id() != interaction_id {
        return Err(format!(
            "question response id mismatch: outer '{interaction_id}', body '{}'",
            response.question_id()
        ));
    }
    let (info, terminal_sink) = {
        let mut h = hub.lock().await;
        if let Some(expected) = remote_uplink
            && !h
                .uplink
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(active, expected))
        {
            return Err("remote question response arrived on a stale uplink generation".into());
        }
        let key = h
            .pending_questions
            .iter()
            .find(|((agent, _), info)| {
                agent == agent_name
                    && info.interaction_id == interaction_id
                    && match (&info.audience, remote_uplink) {
                        (InteractionAudience::LocalUi, None) => true,
                        (InteractionAudience::RemoteUi { uplink, .. }, Some(expected)) => {
                            Arc::ptr_eq(uplink, expected)
                        }
                        _ => false,
                    }
            })
            .map(|(key, _)| key.clone());
        let info = key.and_then(|key| h.pending_questions.remove(&key));
        (info, TerminalEventSink::from_hub(&h))
    };
    let Some(info) = info else {
        return Ok(false);
    };
    info!(
        agent = %info.agent_name,
        logical_id = %info.logical_id,
        interaction_id,
        kind = ?std::mem::discriminant(&response),
        "question resolved"
    );
    let response = rewrite_question_id(response, &info.logical_id);
    let resp = serde_json::to_value(&response).unwrap_or(serde_json::Value::Null);
    let resolved = AgentEvent::named(
        QualifiedAddress::local(&info.agent_name),
        AgentEventPayload::UserQuestionResolved {
            id: info.interaction_id.clone(),
            by: ResolveSource::Manual,
        },
    );
    complete_detached(
        info.agent_conn,
        info.agent_ipc_id,
        resp,
        Some((terminal_sink, resolved)),
    );
    Ok(true)
}

fn rewrite_question_id(
    response: loopal_protocol::UserQuestionResponse,
    logical_id: &str,
) -> loopal_protocol::UserQuestionResponse {
    use loopal_protocol::UserQuestionResponse;
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            UserQuestionResponse::answered(logical_id, answers)
        }
        UserQuestionResponse::Cancelled { .. } => UserQuestionResponse::cancelled(logical_id),
        UserQuestionResponse::Unsupported { reason, .. } => {
            UserQuestionResponse::unsupported(logical_id, reason)
        }
    }
}
