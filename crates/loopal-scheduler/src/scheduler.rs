//! `CronScheduler` — central state and lifecycle.
//!
//! This module owns the struct definition, in-memory and session-scoped
//! constructors, and the background tick loop entrypoint. CRUD operations
//! (`add` / `remove` / `list`) live in [`crate::scheduler_crud`] and the
//! legacy single-path `DurableStore` adapter is in [`crate::scheduler_legacy`].
//!
//! ## Lock ordering
//!
//! Two locks must always be acquired in the same order to prevent
//! deadlock between [`switch_session`](crate::scheduler_session) and
//! the persistence path:
//!
//!   1. `tasks` (read or write)
//!   2. `active`
//!
//! `switch_session`, `persist_locked`, and `load_persisted` all observe
//! this order.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{Mutex, RwLock, broadcast};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::clock::{Clock, SystemClock};
use crate::persistence_session::SessionScopedCronStorage;
use crate::task::ScheduledTask;
use crate::trigger::ScheduledTrigger;

/// Maximum number of concurrent scheduled tasks.
pub(crate) const MAX_TASKS: usize = 50;

/// Capacity for the change-notification broadcast channel.
///
/// Mirrors `loopal_agent::task_store::BROADCAST_CAPACITY`. Each subscriber
/// gets an independent receiver; lagging consumers fall behind by at
/// most this many pulses before getting `Lagged` and a forced
/// re-snapshot, which is harmless because every pulse means
/// "something changed — re-list".
pub const BROADCAST_CAPACITY: usize = 16;

/// Currently bound storage and session id.
///
/// `session_id` is `None` immediately after a session-scoped storage is
/// attached but before [`CronScheduler::switch_session`](crate::CronScheduler::switch_session)
/// is called — in that state persistence is a no-op (no session to save
/// under). Once bound, it carries `Some(id)`, and all `load`/`save_all`
/// calls flow through that id.
///
/// The legacy [`CronScheduler::with_store`](crate::CronScheduler::with_store)
/// path constructs a binding with `session_id = Some(String::new())`
/// plus a [`LegacyBindAdapter`](crate::scheduler_legacy::LegacyBindAdapter)
/// that ignores the id, so existing single-path callers keep working.
pub(crate) struct ActiveBinding {
    pub(crate) session_id: Option<String>,
    pub(crate) storage: Arc<dyn SessionScopedCronStorage>,
}

/// Cron-based scheduler that manages tasks and emits triggers.
///
/// Thread-safe — task list under async `RwLock`, persistence binding
/// under async `Mutex`. See module-level docs for lock ordering rules.
///
/// Job-set changes (add / remove / switch_session / one-shot fire-and-
/// remove) are advertised on a `tokio::sync::broadcast` channel
/// `change_tx`. Subscribers re-snapshot on each pulse — the bridge
/// layer no longer polls.
pub struct CronScheduler {
    pub(crate) tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    pub(crate) started: AtomicBool,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) active: Arc<Mutex<Option<ActiveBinding>>>,
    /// Set when a `save_all` call fails. The next tick (or next mutation)
    /// retries so transient disk errors don't permanently diverge
    /// memory from disk.
    pub(crate) dirty: Arc<AtomicBool>,
    /// Latched when the durable store refuses to operate (e.g. a
    /// corrupt file could not be quarantined). While set, all
    /// `persist_locked` calls become no-ops so the scheduler never
    /// overwrites unrecognized on-disk state. In-memory operation
    /// continues uninterrupted.
    pub(crate) store_disabled: Arc<AtomicBool>,
    /// Broadcast channel signalling job-set changes. Capacity is small
    /// (`BROADCAST_CAPACITY`) — pulses are coalesce-able by the consumer.
    pub(crate) change_tx: broadcast::Sender<()>,
}

impl CronScheduler {
    /// Create an in-memory-only scheduler with the default system clock.
    pub fn new() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }

    /// Create an in-memory-only scheduler with a custom clock.
    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        let (change_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
            started: AtomicBool::new(false),
            clock,
            active: Arc::new(Mutex::new(None)),
            dirty: Arc::new(AtomicBool::new(false)),
            store_disabled: Arc::new(AtomicBool::new(false)),
            change_tx,
        }
    }

    /// Create a scheduler backed by a session-scoped `storage`. The
    /// scheduler is unbound until
    /// [`switch_session`](crate::CronScheduler::switch_session) is called
    /// — `add` calls before that point write to memory only.
    pub fn with_session_storage(storage: Arc<dyn SessionScopedCronStorage>) -> Self {
        Self::with_session_storage_and_clock(storage, Arc::new(SystemClock))
    }

    /// Like [`with_session_storage`](Self::with_session_storage) but with
    /// a custom clock (for deterministic testing).
    pub fn with_session_storage_and_clock(
        storage: Arc<dyn SessionScopedCronStorage>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let (change_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
            started: AtomicBool::new(false),
            clock,
            active: Arc::new(Mutex::new(Some(ActiveBinding {
                session_id: None,
                storage,
            }))),
            dirty: Arc::new(AtomicBool::new(false)),
            store_disabled: Arc::new(AtomicBool::new(false)),
            change_tx,
        }
    }

    /// Subscribe to job-set change notifications. Each call returns an
    /// independent `broadcast::Receiver`; multiple subscribers (the
    /// bridge layer, future metrics observers, …) coexist without
    /// interfering. A pulse means "the cron job set changed — re-list".
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.change_tx.subscribe()
    }

    pub(crate) fn notify_change(&self) {
        // `send()` errors only when there are no receivers — that's
        // expected during the windowed period between scheduler
        // construction and any subscriber connecting. Drop the error.
        let _ = self.change_tx.send(());
    }

    /// Start the background tick loop. Fires triggers on `trigger_tx`.
    ///
    /// # Panics
    /// Panics if called more than once on the same scheduler.
    pub fn start(
        &self,
        trigger_tx: tokio::sync::mpsc::Sender<ScheduledTrigger>,
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
        };
        tokio::spawn(crate::tick::tick_loop(ctx, trigger_tx, cancel))
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}
