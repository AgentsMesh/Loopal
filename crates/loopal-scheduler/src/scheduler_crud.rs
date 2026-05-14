use std::sync::atomic::Ordering;

use crate::error::SchedulerError;
use crate::expression::CronExpression;
use crate::id::{find_unique_id, generate_task_id};
use crate::scheduler::{CronScheduler, MAX_TASKS};
use crate::task::{CronJobInfo, ScheduledTask};

impl CronScheduler {
    /// Returns the 8-char task ID. Durable tasks are persisted before
    /// this method returns when the scheduler is session-bound.
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

    /// Returns `true` if found and removed. A durable removal is written
    /// through to the store inline.
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

    pub async fn list(&self) -> Vec<CronJobInfo> {
        let tasks = self.tasks.read().await;
        let now = self.clock.now();
        tasks
            .iter()
            .map(|t| {
                let next_fire = t.next_fire().and_then(|next| {
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
