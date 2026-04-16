//! Task snapshot state updates from `TasksChanged` events.

use loopal_protocol::{AgentEventPayload, TaskSnapshot};

use crate::state::SessionState;

pub(crate) fn apply(state: &mut SessionState, payload: AgentEventPayload) {
    if let AgentEventPayload::TasksChanged { tasks } = payload {
        state.task_snapshots = tasks;
    }
}

/// Check whether any non-completed tasks exist in the snapshot.
pub fn has_active_tasks(snapshots: &[TaskSnapshot]) -> bool {
    use loopal_protocol::TaskSnapshotStatus;
    snapshots
        .iter()
        .any(|t| !matches!(t.status, TaskSnapshotStatus::Completed))
}
