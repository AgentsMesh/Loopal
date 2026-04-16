//! Task event bridge — subscribes to TaskStore change notifications
//! and emits `TasksChanged` events via the Hub frontend.
//!
//! Snapshot conversion (Task → TaskSnapshot) lives here, not in TaskStore,
//! keeping the store as a pure persistence layer.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_agent::task_store::TaskStore;
use loopal_agent::types::{Task, TaskStatus};
use loopal_protocol::{AgentEventPayload, TaskSnapshot, TaskSnapshotStatus};
use loopal_runtime::frontend::traits::AgentFrontend;

pub fn spawn(
    mut change_rx: mpsc::UnboundedReceiver<()>,
    task_store: Arc<TaskStore>,
    frontend: Arc<dyn AgentFrontend>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while change_rx.recv().await.is_some() {
            while change_rx.try_recv().is_ok() {}
            let tasks = snapshot_all(&task_store);
            if let Err(e) = frontend
                .emit(AgentEventPayload::TasksChanged { tasks })
                .await
            {
                tracing::warn!(error = %e, "failed to emit TasksChanged");
            }
        }
    })
}

fn snapshot_all(store: &TaskStore) -> Vec<TaskSnapshot> {
    store
        .list()
        .into_iter()
        .filter(|t| !matches!(t.status, TaskStatus::Completed))
        .map(|t| task_to_snapshot(&t))
        .collect()
}

fn task_to_snapshot(task: &Task) -> TaskSnapshot {
    let status = match task.status {
        TaskStatus::Pending => TaskSnapshotStatus::Pending,
        TaskStatus::InProgress => TaskSnapshotStatus::InProgress,
        // Unreachable: list() excludes Deleted, snapshot_all() excludes Completed
        TaskStatus::Completed | TaskStatus::Deleted => TaskSnapshotStatus::Completed,
    };
    TaskSnapshot {
        id: task.id.clone(),
        subject: task.subject.replace('\n', " ").replace('\r', ""),
        active_form: task.active_form.clone(),
        status,
        blocked_by: task.blocked_by.clone(),
    }
}
