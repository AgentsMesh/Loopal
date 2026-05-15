//! Lock ordering — both locks must always be acquired in this order to
//! prevent deadlock between `switch_session` and the persistence path:
//!
//!   1. `tasks` (read or write)
//!   2. `active`
//!
//! `switch_session`, `persist_locked`, and `load_persisted` all observe
//! this order.
//!
//! Disk I/O ordering: all `save_all` calls are funneled through a
//! single background worker via `save_tx`. Mutation paths snapshot
//! durable tasks under `tasks.write`, then push the snapshot to the
//! channel and release the lock — the worker runs `save_all` (including
//! fsync) outside any task lock. Channel arrival order equals lock
//! release order, so the disk state always reflects the most recent
//! in-memory mutation without `tasks.write` ever waiting on disk I/O.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::clock::{Clock, SystemClock};
use crate::persistence_session::SessionScopedCronStorage;
use crate::save_worker::{SaveMessage, save_worker_loop};
use crate::task::ScheduledTask;
use crate::trigger::ScheduledTrigger;

/// User-facing — `CronCreateTool::description()` in
/// `loopal-agent/src/tools/cron.rs` mirrors this number ("Max 50
/// concurrent jobs"). Update both together.
pub(crate) const MAX_TASKS: usize = 50;

/// Maximum prompt length accepted by `CronScheduler::add` (counted in
/// Unicode scalar values, not bytes). Cron tools validate against this
/// before forwarding.
pub const MAX_PROMPT_CHARS: usize = 4096;

/// Capacity of the job-set change-notification broadcast channel.
/// Lagging consumers fall behind by at most this many pulses before
/// getting `Lagged` and a forced re-snapshot — harmless because every
/// pulse means "the cron job set changed; re-list".
pub const BROADCAST_CAPACITY: usize = 16;

/// Save-worker channel buffer. Sized so a slow disk (fsync stalling for
/// seconds) does not back-pressure mutation paths under bursty load.
const SAVE_QUEUE_CAPACITY: usize = 64;

/// `session_id` is `None` immediately after a session-scoped storage is
/// attached but before `switch_session` is called — persistence is a
/// no-op until the binding has both `storage` and `session_id`.
pub(crate) struct ActiveBinding {
    pub(crate) session_id: Option<String>,
    pub(crate) storage: Arc<dyn SessionScopedCronStorage>,
}

pub struct CronScheduler {
    pub(crate) tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    pub(crate) started: AtomicBool,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) active: Arc<Mutex<Option<ActiveBinding>>>,
    /// Set when a `save_all` call fails (by the save worker). The next
    /// tick (or next mutation) retries so transient disk errors don't
    /// permanently diverge memory from disk.
    pub(crate) dirty: Arc<AtomicBool>,
    /// Latched when the durable store refuses to operate (e.g. a corrupt
    /// file could not be quarantined). While set, mutation paths skip
    /// emitting save requests so the worker never overwrites the
    /// unrecognized on-disk state.
    pub(crate) store_disabled: Arc<AtomicBool>,
    pub(crate) change_tx: broadcast::Sender<()>,
    /// Sender side of the single-worker save queue. Cloned into every
    /// mutation path; the worker side is owned by a background task
    /// that lives for the lifetime of this scheduler.
    pub(crate) save_tx: mpsc::Sender<SaveMessage>,
}

impl CronScheduler {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }

    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        Self::build(clock, None)
    }

    /// `add` calls before `switch_session` write to memory only.
    pub fn with_session_storage(storage: Arc<dyn SessionScopedCronStorage>) -> Self {
        Self::with_session_storage_and_clock(storage, Arc::new(SystemClock))
    }

    pub fn with_session_storage_and_clock(
        storage: Arc<dyn SessionScopedCronStorage>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self::build(clock, Some(storage))
    }

    fn build(clock: Arc<dyn Clock>, storage: Option<Arc<dyn SessionScopedCronStorage>>) -> Self {
        let (change_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (save_tx, save_rx) = mpsc::channel(SAVE_QUEUE_CAPACITY);
        // One idle tokio task per scheduler is the durability cost —
        // multi-agent processes pay N workers worth of overhead. Each
        // worker stays parked on `rx.recv()` until a save lands, so
        // memory residence is small but task-count scales linearly
        // with concurrent schedulers.
        //
        // Only spawn when we're already inside a tokio runtime. Outside
        // a runtime (e.g. sync `#[test]` helpers constructing scratch
        // schedulers) the receiver drops here, and any later
        // `save_tx.send` returns `Closed` — handled by mutation paths.
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::spawn(save_worker_loop(save_rx));
        }
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
            started: AtomicBool::new(false),
            clock,
            active: Arc::new(Mutex::new(storage.map(|s| ActiveBinding {
                session_id: None,
                storage: s,
            }))),
            dirty: Arc::new(AtomicBool::new(false)),
            store_disabled: Arc::new(AtomicBool::new(false)),
            change_tx,
            save_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.change_tx.subscribe()
    }

    pub(crate) fn notify_change(&self) {
        // send() errors only when no receivers are attached yet — drop it.
        let _ = self.change_tx.send(());
    }

    /// Wait until every save request enqueued before this call has been
    /// processed by the background worker. Useful for tests and tooling
    /// that need a synchronization point against on-disk state without
    /// polling. No-op if the worker channel has closed.
    ///
    /// **Warning**: This method blocks until all pending saves complete.
    /// Do NOT use in production startup paths — disk I/O latency would
    /// block handshake and cause timeouts. Use only in tests or tooling.
    pub async fn wait_idle(&self) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self.save_tx.send(SaveMessage::Barrier(tx)).await.is_err() {
            return;
        }
        let _ = rx.await;
    }

    /// # Panics
    /// Panics if called more than once on the same scheduler.
    pub fn start(
        &self,
        trigger_tx: mpsc::Sender<ScheduledTrigger>,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        assert!(
            !self.started.swap(true, Ordering::SeqCst),
            "CronScheduler::start() called more than once"
        );
        let ctx = crate::tick_context::TickContext {
            tasks: self.tasks.clone(),
            clock: self.clock.clone(),
            active: self.active.clone(),
            dirty: self.dirty.clone(),
            store_disabled: self.store_disabled.clone(),
            change_tx: self.change_tx.clone(),
            save_tx: self.save_tx.clone(),
        };
        tokio::spawn(crate::tick::tick_loop(ctx, trigger_tx, cancel))
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}
