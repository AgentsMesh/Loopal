use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::warn;

use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};

use crate::hub::Hub;

/// Drop every pending permission/question request belonging to `agent_name`
/// and emit `Resolved` events so attached UI dialogs clear immediately.
/// Called from `finish_and_deliver` when an agent IO loop exits — the only
/// path that ever cleans pending entries that the UI never responded to.
pub(crate) async fn cleanup_pending_for_agent(hub: &Arc<Mutex<Hub>>, agent_name: &str) {
    let (perm_ids, question_ids) = {
        let mut h = hub.lock().await;
        let perm_ids: Vec<String> = h
            .pending_permissions
            .keys()
            .filter(|(a, _)| a == agent_name)
            .map(|(_, id)| id.clone())
            .collect();
        for id in &perm_ids {
            h.pending_permissions
                .remove(&(agent_name.to_string(), id.clone()));
        }
        let question_ids: Vec<String> = h
            .pending_questions
            .keys()
            .filter(|(a, _)| a == agent_name)
            .map(|(_, id)| id.clone())
            .collect();
        for id in &question_ids {
            h.pending_questions
                .remove(&(agent_name.to_string(), id.clone()));
        }
        (perm_ids, question_ids)
    };

    if perm_ids.is_empty() && question_ids.is_empty() {
        return;
    }

    let event_tx = hub.lock().await.registry.event_sender();
    let address = QualifiedAddress::local(agent_name);
    for id in perm_ids {
        let event = AgentEvent::named(
            address.clone(),
            AgentEventPayload::ToolPermissionResolved { id },
        );
        if event_tx.try_send(event).is_err() {
            warn!(agent = %agent_name, "stranded ToolPermissionResolved dropped");
        }
    }
    for id in question_ids {
        let event = AgentEvent::named(
            address.clone(),
            AgentEventPayload::UserQuestionResolved { id },
        );
        if event_tx.try_send(event).is_err() {
            warn!(agent = %agent_name, "stranded UserQuestionResolved dropped");
        }
    }
}
