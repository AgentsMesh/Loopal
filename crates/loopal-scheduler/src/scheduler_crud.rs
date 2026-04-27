//! CRUD operations on [`CronScheduler`]: `add` / `remove` / `list`.
//!
//! Split out so [`crate::scheduler`] keeps its focus on lifecycle
//! (constructors, `start`) and persistence integration lives in
//! [`crate::scheduler_persistence`].

use std::sync::atomic::Ordering;

use crate::error::{SchedulerError, generate_task_id};
use crate::expression::CronExpression;
use crate::id::find_unique_id;
use crate::scheduler::{CronScheduler, MAX_TASKS};
use crate::task::{CronJobInfo, ScheduledTask, truncate_to_secs};

impl CronScheduler {
    /// Add a new scheduled task. Returns the 8-char task ID.
    ///
    /// When `durable` is `true` and the scheduler is bound to a session
    /// (or attached to a legacy single-path store), the new task is
    /// persisted before this method returns.
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
        drop(tasks);
        self.notify_change();
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
        drop(tasks);
        if removed {
            self.notify_change();
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
}
