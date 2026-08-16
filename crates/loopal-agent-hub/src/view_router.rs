//! UI ↔ Hub `view/snapshot` handler.
//!
//! UI clients seed their local replica from `view/snapshot` and then
//! follow the existing `agent/event` notification broadcast — the Hub
//! does not push separate `view/delta` notifications.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Mutex;

use loopal_protocol::{AgentEventPayload, ResolveSource};
use loopal_view_state::{ViewSnapshot, ViewSnapshotRequest, ViewStateReducer};

use crate::hub::Hub;
use crate::workflow::{WorkflowCoordinatorHandle, WorkflowOwner, owner_for_managed_root};

type View = Arc<Mutex<ViewStateReducer>>;

enum SnapshotTarget {
    Local {
        view: View,
        workflow: Option<(WorkflowOwner, WorkflowCoordinatorHandle)>,
    },
    Remote(View),
}

/// Handle `view/snapshot`. Returns the JSON-serialized `ViewSnapshot`
/// (or an error if the agent is not registered).
pub async fn handle_snapshot(hub: &Arc<Mutex<Hub>>, params: Value) -> Result<Value, String> {
    let req: ViewSnapshotRequest =
        serde_json::from_value(params).map_err(|e| format!("malformed view/snapshot: {e}"))?;
    let target = {
        let h = hub.lock().await;
        if let Some(view) = h.registry.agent_view(&req.agent) {
            SnapshotTarget::Local {
                view,
                workflow: workflow_authority(&h, &req.agent),
            }
        } else {
            let view = h
                .remote_views
                .get(&req.agent)
                .cloned()
                .ok_or_else(|| format!("no agent: '{}'", req.agent))?;
            SnapshotTarget::Remote(view)
        }
    };
    let snapshot = match target {
        SnapshotTarget::Local { view, workflow } => {
            local_snapshot(&view, workflow, &req.agent).await
        }
        SnapshotTarget::Remote(view) => remote_snapshot(hub, &view, &req.agent).await,
    };
    serde_json::to_value(&snapshot).map_err(|e| format!("serialize snapshot: {e}"))
}

fn workflow_authority(
    hub: &Hub,
    agent: &str,
) -> Option<(WorkflowOwner, WorkflowCoordinatorHandle)> {
    let execution = hub.registry.current_execution(agent)?;
    if !hub.registry.owns_active_lease(&execution) {
        return None;
    }
    let facts = hub.registry.runtime_facts(&execution)?;
    let owner = owner_for_managed_root(&execution, facts).ok()?;
    Some((owner, hub.workflow_coordinator()?))
}

async fn local_snapshot(
    view: &View,
    workflow: Option<(WorkflowOwner, WorkflowCoordinatorHandle)>,
    agent: &str,
) -> ViewSnapshot {
    let authoritative = match workflow {
        Some((owner, coordinator)) => match coordinator.snapshot(owner).await {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                tracing::warn!(agent, %error, "authoritative workflow snapshot unavailable");
                None
            }
        },
        None => None,
    };
    let mut snapshot = view.lock().await.snapshot();
    if let Some(workflows) = authoritative {
        snapshot.state.workflows = workflows;
    }
    snapshot
}

async fn remote_snapshot(hub: &Arc<Mutex<Hub>>, view: &View, agent: &str) -> ViewSnapshot {
    let mut view = view.lock().await;
    // Keep authority stable while reconciling. The event router releases
    // the Hub lock before taking a reducer lock, so this order cannot cycle.
    let h = hub.lock().await;
    let authoritative = h
        .pending_remote_questions
        .values()
        .find(|record| record.qualified_agent == agent)
        .map(|record| record.request.clone());
    reconcile_remote_question(&mut view, authoritative);
    view.snapshot()
}

#[cfg(test)]
#[path = "view_router_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "view_router_workflow_tests.rs"]
mod workflow_tests;

#[cfg(test)]
#[path = "view_router_lock_tests.rs"]
mod lock_tests;

#[cfg(test)]
#[path = "view_router_test_support.rs"]
mod test_support;

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
