//! Task snapshot state updates from `TasksChanged` / `SessionResumed` events.

use loopal_protocol::{AgentEventPayload, TaskSnapshot};

use crate::state::SessionState;

pub(crate) fn apply(state: &mut SessionState, payload: AgentEventPayload) {
    match payload {
        AgentEventPayload::TasksChanged { tasks } => {
            state.task_snapshots = tasks;
        }
        AgentEventPayload::SessionResumed { .. } => {
            // Clear stale task snapshots from the prior session; the task
            // bridge will re-emit against the resumed TaskStore on its next
            // change notification.
            state.task_snapshots.clear();
        }
        _ => {}
    }
}

/// Check whether any non-completed tasks exist in the snapshot.
pub fn has_active_tasks(snapshots: &[TaskSnapshot]) -> bool {
    use loopal_protocol::TaskSnapshotStatus;
    snapshots
        .iter()
        .any(|t| !matches!(t.status, TaskSnapshotStatus::Completed))
}
