//! Task event bridge — subscribes to TaskStore change notifications
//! and emits `TasksChanged` events via the Hub frontend.
//!
//! Uses `tokio::sync::broadcast` for the change channel: multiple
//! subscribers (this bridge, future metrics observers, …) can coexist
//! without overwriting each other. On `Lagged` we re-snapshot rather
//! than fail — losing intermediate "something changed" pulses is fine
//! because each pulse triggers the same `list()` call anyway.
//!
//! Snapshot conversion (`Task` → `TaskSnapshot`) lives in
//! `loopal_agent::state_snapshot` so this bridge, `AgentShared::snapshot_state`,
//! and any future observer share one definition.

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use loopal_agent::task_store::TaskStore;
use loopal_agent::task_to_snapshot;
use loopal_agent::types::TaskStatus;
use loopal_protocol::{AgentEventPayload, TaskSnapshot};
use loopal_runtime::frontend::traits::AgentFrontend;

pub fn spawn(
    mut change_rx: broadcast::Receiver<()>,
    task_store: Arc<TaskStore>,
    frontend: Arc<dyn AgentFrontend>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match change_rx.recv().await {
                Ok(()) => {}
                // Lagged: we missed N pulses but each pulse means
                // "something changed" — a single re-snapshot covers all
                // of them, so the sensible recovery is to fall through
                // and emit one TasksChanged.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    tracing::warn!("task_bridge lagged; re-snapshotting");
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
            let tasks = snapshot_all(&task_store).await;
            if let Err(e) = frontend
                .emit(AgentEventPayload::TasksChanged { tasks })
                .await
            {
                tracing::warn!(error = %e, "failed to emit TasksChanged");
            }
        }
    })
}

async fn snapshot_all(store: &TaskStore) -> Vec<TaskSnapshot> {
    store
        .list()
        .await
        .into_iter()
        .filter(|t| !matches!(t.status, TaskStatus::Completed))
        .map(|t| task_to_snapshot(&t))
        .collect()
}
