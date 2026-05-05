//! Session state cleanup — prevents unbounded growth in long-running sessions.

use indexmap::IndexMap;
use loopal_protocol::AgentStatus;

use crate::view_client::ViewClient;

/// Sweep finished sub-agents out of the per-agent ViewClient map.
///
/// The active view and root "main" are always preserved. Finished/errored
/// agents are removed only after they've been observed as terminal —
/// their final state has already been mirrored into `bg_task_details`
/// where applicable.
pub(crate) fn cleanup_view_clients(view_clients: &mut IndexMap<String, ViewClient>, active: &str) {
    let removable: Vec<String> = view_clients
        .iter()
        .filter(|(name, vc)| {
            if name.as_str() == "main" || name.as_str() == active {
                return false;
            }
            let status = vc.state().state().agent.observable.status;
            matches!(status, AgentStatus::Finished | AgentStatus::Error)
        })
        .map(|(name, _)| name.clone())
        .collect();
    for name in removable {
        view_clients.shift_remove(&name);
    }
}

const MAX_BG_DETAIL_ARCHIVE: usize = 50;

/// Variant fed by the local `ViewClient` instead of `SessionState`.
/// Each tuple is `(id, description, status, exit_code, output)` —
/// matches the projection done in `tui_loop::sync_panel_caches`.
pub(crate) fn merge_bg_details_from_view(
    details: &mut Vec<loopal_protocol::BgTaskDetail>,
    source: &[(
        String,
        String,
        loopal_protocol::BgTaskStatus,
        Option<i32>,
        String,
    )],
) {
    for (id, description, status, exit_code, output) in source {
        let detail = loopal_protocol::BgTaskDetail {
            id: id.clone(),
            description: description.clone(),
            status: *status,
            exit_code: *exit_code,
            output: output.clone(),
        };
        if let Some(existing) = details.iter_mut().find(|d| d.id == *id) {
            *existing = detail;
        } else {
            details.push(detail);
        }
    }
}

pub(crate) fn cap_bg_details(details: &mut Vec<loopal_protocol::BgTaskDetail>) {
    if details.len() > MAX_BG_DETAIL_ARCHIVE {
        details.drain(..details.len() - MAX_BG_DETAIL_ARCHIVE);
    }
}
