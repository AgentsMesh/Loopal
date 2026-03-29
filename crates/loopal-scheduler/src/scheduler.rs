use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::clock::{Clock, SystemClock};
use crate::error::{SchedulerError, generate_task_id};
use crate::expression::CronExpression;
use crate::task::{CronJobInfo, ScheduledTask, truncate_to_secs};
use crate::trigger::ScheduledTrigger;

/// Maximum number of concurrent scheduled tasks.
const MAX_TASKS: usize = 50;

/// Cron-based scheduler that manages tasks and emits triggers.
///
/// Thread-safe — all mutation goes through an internal `Mutex`.
/// The background tick loop is started via [`start()`](Self::start).
pub struct CronScheduler {
    tasks: Arc<Mutex<Vec<ScheduledTask>>>,
    started: AtomicBool,
    clock: Arc<dyn Clock>,
}

impl CronScheduler {
    /// Create a scheduler with the default system clock.
    pub fn new() -> Self {
        Self::with_clock(Arc::new(SystemClock))
    }

    /// Create a scheduler with a custom clock (for deterministic testing).
    pub fn with_clock(clock: Arc<dyn Clock>) -> Self {
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            started: AtomicBool::new(false),
            clock,
        }
    }

    /// Add a new scheduled task. Returns the 8-char task ID.
    pub async fn add(
        &self,
        cron_expr: &str,
        prompt: &str,
        recurring: bool,
    ) -> Result<String, SchedulerError> {
        let now = self.clock.now();
        let cron = CronExpression::parse_at(cron_expr, now).map_err(SchedulerError::InvalidCron)?;
        let mut tasks = self.tasks.lock().await;
        if tasks.len() >= MAX_TASKS {
            return Err(SchedulerError::TooManyTasks(MAX_TASKS));
        }
        let mut id = generate_task_id();
        while tasks.iter().any(|t| t.id == id) {
            id = generate_task_id();
        }
        tasks.push(ScheduledTask {
            id: id.clone(),
            cron,
            prompt: prompt.to_string(),
            recurring,
            created_at: now,
            last_fired: None,
        });
        Ok(id)
    }

    /// Remove a task by ID. Returns `true` if found and removed.
    pub async fn remove(&self, id: &str) -> bool {
        let mut tasks = self.tasks.lock().await;
        let before = tasks.len();
        tasks.retain(|t| t.id != id);
        tasks.len() < before
    }

    /// List all active tasks as read-only snapshots.
    pub async fn list(&self) -> Vec<CronJobInfo> {
        let tasks = self.tasks.lock().await;
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
        tokio::spawn(async move {
            crate::tick::tick_loop(tasks, trigger_tx, cancel, clock).await;
        })
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}
