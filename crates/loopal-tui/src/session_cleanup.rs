//! Session state cleanup — prevents unbounded growth in long-running sessions.

use loopal_protocol::{AgentStatus, BgTaskStatus};
use loopal_session::state::SessionState;

/// Remove completed bg_tasks and finished sub-agents from session state.
///
/// Called each frame after panel data has been synced to App-level caches.
/// Safe because: panel already filters by status, log viewer uses App cache,
/// topology filters live agents only.
pub(crate) fn cleanup_session_state(state: &mut SessionState) {
    state
        .bg_tasks
        .retain(|_, t| t.status == BgTaskStatus::Running);
    cleanup_finished_agents(state);
}

fn cleanup_finished_agents(state: &mut SessionState) {
    let active = state.active_view.clone();
    let removable: Vec<String> = state
        .agents
        .iter()
        .filter(|(name, a)| {
            let is_root = *name == loopal_session::ROOT_AGENT;
            let is_active = *name == &active;
            let is_finished = matches!(
                a.observable.status,
                AgentStatus::Finished | AgentStatus::Error
            );
            // Only remove if session was persisted. Sub-agents spawned via Hub
            // always receive a session_id; agents without one are transient
            // and will be cleaned on next spawn cycle.
            !is_root && !is_active && is_finished && a.session_id.is_some()
        })
        .map(|(name, _)| name.clone())
        .collect();
    for name in removable {
        state.agents.shift_remove(&name);
    }
}

const MAX_BG_DETAIL_ARCHIVE: usize = 50;

pub(crate) fn merge_bg_details(
    details: &mut Vec<loopal_protocol::BgTaskDetail>,
    source: &indexmap::IndexMap<String, loopal_protocol::BgTaskDetail>,
) {
    for (id, detail) in source {
        if let Some(existing) = details.iter_mut().find(|d| d.id == *id) {
            *existing = detail.clone();
        } else {
            details.push(detail.clone());
        }
    }
}

pub(crate) fn cap_bg_details(details: &mut Vec<loopal_protocol::BgTaskDetail>) {
    if details.len() > MAX_BG_DETAIL_ARCHIVE {
        details.drain(..details.len() - MAX_BG_DETAIL_ARCHIVE);
    }
}
