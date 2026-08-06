//! UI ↔ Hub `view/snapshot` handler.
//!
//! UI clients seed their local replica from `view/snapshot` and then
//! follow the existing `agent/event` notification broadcast — the Hub
//! does not push separate `view/delta` notifications.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Mutex;

use loopal_protocol::{AgentEventPayload, ResolveSource};
use loopal_view_state::ViewSnapshotRequest;

use crate::hub::Hub;

/// Handle `view/snapshot`. Returns the JSON-serialized `ViewSnapshot`
/// (or an error if the agent is not registered).
pub async fn handle_snapshot(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let req: ViewSnapshotRequest =
        serde_json::from_value(params).map_err(|e| format!("malformed view/snapshot: {e}"))?;
    let (view, is_remote) = {
        let h = hub.lock().await;
        if let Some(view) = h.registry.agent_view(&req.agent) {
            (view, false)
        } else {
            let view = h
                .remote_views
                .get(&req.agent)
                .cloned()
                .ok_or_else(|| format!("no agent: '{}'", req.agent))?;
            (view, true)
        }
    };
    let mut view = view.lock().await;
    if is_remote {
        // Keep authority stable while reconciling. The event router releases
        // the Hub lock before taking a reducer lock, so this order cannot cycle.
        let h = hub.lock().await;
        let authoritative = h
            .pending_remote_questions
            .values()
            .find(|record| record.qualified_agent == req.agent)
            .map(|record| record.request.clone());
        reconcile_remote_question(&mut view, authoritative);
    }
    let snapshot = view.snapshot();
    serde_json::to_value(&snapshot).map_err(|e| format!("serialize snapshot: {e}"))
}

#[cfg(test)]
#[path = "view_router_tests.rs"]
mod tests;

fn reconcile_remote_question(
    view: &mut loopal_view_state::ViewStateReducer,
    authoritative: Option<AgentEventPayload>,
) {
    let pending_id = view
        .state()
        .agent
        .conversation
        .pending_question
        .as_ref()
        .map(|question| question.id.clone());
    match authoritative {
        Some(authoritative) => {
            let AgentEventPayload::UserQuestionRequest { id, .. } = &authoritative else {
                return;
            };
            if pending_id.as_deref() != Some(id) {
                view.apply(authoritative);
            }
        }
        None => {
            if let Some(id) = pending_id {
                view.apply(AgentEventPayload::UserQuestionResolved {
                    id,
                    by: ResolveSource::Agent,
                });
            }
        }
    }
}
