use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::clock::{Clock, SystemClock};
use crate::error::{SchedulerError, generate_task_id};
use crate::expression::CronExpression;
use crate::id::find_unique_id;
use crate::persistence::DurableStore;
use crate::task::{CronJobInfo, ScheduledTask, truncate_to_secs};
use crate::trigger::ScheduledTrigger;

/// Maximum number of concurrent scheduled tasks.
pub(crate) const MAX_TASKS: usize = 50;

/// Cron-based scheduler that manages tasks and emits triggers.
///
/// Thread-safe — the task list is guarded by an async `RwLock` so that
/// concurrent read-only callers (`list()`) can proceed without blocking
/// each other. Writers (`add`, `remove`, and the internal tick loop when
/// it fires or expires a task) acquire an exclusive lock.
///
/// When a [`DurableStore`] is attached via [`with_store`](Self::with_store)
/// or [`with_store_and_clock`](Self::with_store_and_clock), mutations
/// that touch `durable = true` tasks are written through to the store
/// while the `tasks` write lock is held, preventing interleaved saves
/// from producing a stale on-disk view.
pub struct CronScheduler {
    pub(crate) tasks: Arc<RwLock<Vec<ScheduledTask>>>,
    started: AtomicBool,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) store: Option<Arc<dyn DurableStore>>,
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
}

impl CronScheduler {
    /// Create an in-memory-only scheduler with the default system clock.
    pub fn new() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }

    /// Create an in-memory-only scheduler with a custom clock.
    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
            started: AtomicBool::new(false),
            clock,
            store: None,
            dirty: Arc::new(AtomicBool::new(false)),
            store_disabled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a scheduler backed by `store`. Tasks added with
    /// `durable = true` are written to the store; non-durable tasks
    /// still live only in memory.
    pub fn with_store(store: Arc<dyn DurableStore>) -> Self {
        Self::with_store_and_clock(store, Arc::new(SystemClock))
    }

    /// Like [`with_store`](Self::with_store) but with a custom clock
    /// (for deterministic testing).
    pub fn with_store_and_clock(store: Arc<dyn DurableStore>, clock: Arc<dyn Clock>) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
            started: AtomicBool::new(false),
            clock,
            store: Some(store),
            dirty: Arc::new(AtomicBool::new(false)),
            store_disabled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Add a new scheduled task. Returns the 8-char task ID.
    ///
    /// When `durable` is `true` and the scheduler has a store attached,
    /// the new task is persisted before this method returns.
    pub async fn add(
        &self,
        cron_expr: &str,
        prompt: &str,
        recurring: bool,
        durable: bool,
    ) -> Result<String, SchedulerError> {
        let now = self.clock.now();
        let cron = CronExpression::parse_at(cron_expr, now).map_err(SchedulerError::InvalidCron)?;
        let mut tasks = self.tasks.write().await;
        if tasks.len() >= MAX_TASKS {
            return Err(SchedulerError::TooManyTasks(MAX_TASKS));
        }
        let id = find_unique_id(&tasks, generate_task_id);
        tasks.push(ScheduledTask {
            id: id.clone(),
            cron,
            prompt: prompt.to_string(),
            recurring,
            created_at: now,
            last_fired: None,
            durable,
        });
        if durable || self.dirty.load(Ordering::Acquire) {
            self.persist_locked(&tasks).await;
        }
        Ok(id)
    }

    /// Remove a task by ID. Returns `true` if found and removed.
    ///
    /// A durable removal is written through to the store inline.
    pub async fn remove(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        let was_durable = tasks.iter().any(|t| t.id == id && t.durable);
        let before = tasks.len();
        tasks.retain(|t| t.id != id);
        let removed = tasks.len() < before;
        if removed && (was_durable || self.dirty.load(Ordering::Acquire)) {
            self.persist_locked(&tasks).await;
        }
        removed
    }

    /// List all active tasks as read-only snapshots.
    pub async fn list(&self) -> Vec<CronJobInfo> {
        let tasks = self.tasks.read().await;
        let now = self.clock.now();
        tasks
            .iter()
            .map(|t| {
                let reference = truncate_to_secs(t.last_fired.unwrap_or(t.created_at));
                let next_fire = t.cron.next_after(&reference).and_then(|next| {
                    if next > now {
                        Some(next)
                    } else {
                        t.cron.next_after(&now)
                    }
                });
                CronJobInfo {
                    id: t.id.clone(),
                    cron_expr: t.cron.as_str().to_string(),
                    prompt: t.prompt.clone(),
                    recurring: t.recurring,
                    created_at: t.created_at,
                    next_fire,
                    durable: t.durable,
                }
            })
            .collect()
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
        let tasks = self.tasks.clone();
        let clock = self.clock.clone();
        let store = self.store.clone();
        let dirty = self.dirty.clone();
        let store_disabled = self.store_disabled.clone();
        tokio::spawn(async move {
            crate::tick::tick_loop(
                tasks,
                trigger_tx,
                cancel,
                clock,
                store,
                dirty,
                store_disabled,
            )
            .await;
        })
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}
