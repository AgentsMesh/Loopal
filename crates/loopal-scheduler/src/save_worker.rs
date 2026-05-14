use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{mpsc, oneshot};

use crate::persistence::PersistedTask;
use crate::persistence_session::SessionScopedCronStorage;

/// Message sent to the save worker. `Save` carries a snapshot to write;
/// `Barrier` signals "all prior saves have been processed" — used by
/// `CronScheduler::wait_idle` so tests and tooling can synchronize
/// against disk state without polling.
pub(crate) enum SaveMessage {
    Save(SaveRequest),
    Barrier(oneshot::Sender<()>),
}

/// One save request handed off from a mutation path to the serial worker.
///
/// Worker processes requests in arrival order, which equals the order
/// callers held `tasks.write` when they pushed the snapshot — so disk
/// state always follows the last in-memory mutation, with no races
/// between concurrent save sources (tick, add, remove, switch-session).
pub(crate) struct SaveRequest {
    pub storage: Arc<dyn SessionScopedCronStorage>,
    pub session_id: String,
    pub snapshot: Vec<PersistedTask>,
    pub dirty: Arc<AtomicBool>,
    pub store_disabled: Arc<AtomicBool>,
}

pub(crate) async fn save_worker_loop(mut rx: mpsc::Receiver<SaveMessage>) {
    while let Some(msg) = rx.recv().await {
        match msg {
            SaveMessage::Save(req) => process_save(req).await,
            SaveMessage::Barrier(tx) => {
                let _ = tx.send(());
            }
        }
    }
}

async fn process_save(req: SaveRequest) {
    if req.store_disabled.load(Ordering::Acquire) {
        // Quarantine latched after this request was queued — skip to
        // avoid clobbering an unreadable on-disk file.
        return;
    }
    match req.storage.save_all(&req.session_id, &req.snapshot).await {
        Ok(()) => req.dirty.store(false, Ordering::Release),
        Err(e) => {
            tracing::error!(error = %e, "cron durable save failed in worker");
            req.dirty.store(true, Ordering::Release);
        }
    }
}
