use std::sync::atomic::Ordering;

use crate::persistence::{PersistError, durable_snapshot};
use crate::save_worker::{SaveMessage, SaveRequest};
use crate::scheduler::{CronScheduler, MAX_TASKS};
use crate::task::ScheduledTask;

impl CronScheduler {
    /// Called with `tasks` write lock held. Snapshots the durable
    /// subset and enqueues the save on the background worker — the
    /// actual fsync runs without holding any task lock. Failure to
    /// enqueue (full or closed channel) sets `dirty` so the next tick
    /// retries.
    ///
    /// No-op when no binding, `store_disabled` is latched, or there's
    /// no session yet (the latter prevents clobbering on-disk state
    /// during the brief window between session storage attach and
    /// `switch_session`).
    pub(crate) async fn persist_locked(&self, tasks: &[ScheduledTask]) {
        if self.store_disabled.load(Ordering::Acquire) {
            return;
        }
        let (storage, session_id) = {
            let active = self.active.lock().await;
            let Some(binding) = active.as_ref() else {
                return;
            };
            let Some(sid) = binding.session_id.as_ref() else {
                return;
            };
            (binding.storage.clone(), sid.clone())
        };
        let snapshot = durable_snapshot(tasks);
        let req = SaveRequest {
            storage,
            session_id,
            snapshot,
            dirty: self.dirty.clone(),
            store_disabled: self.store_disabled.clone(),
        };
        // try_send so a full channel doesn't block the caller (which
        // still holds tasks.write). Worst case: dirty=true → next tick
        // retries after the worker drains.
        match self.save_tx.try_send(SaveMessage::Save(req)) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!("cron save worker queue full; deferring to next tick");
                self.dirty.store(true, Ordering::Release);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                tracing::error!(
                    "cron save worker channel closed; latching store_disabled to stop retries"
                );
                self.store_disabled.store(true, Ordering::Release);
            }
        }
    }

    /// Internal — external callers use `switch_session(id)` which
    /// performs flush + clear + load atomically.
    ///
    /// Filter rules (drop-on-load, no catch-up):
    /// - one-shot already fired (`last_fired.is_some()`)
    /// - one-shot whose next fire time has passed
    ///
    /// Normalization: for recurring tasks whose `next_after(reference)`
    /// is already in the past, `last_fired` is clamped to `now` to
    /// prevent an immediate catch-up fire on the next tick.
    ///
    /// Capacity: if the on-disk set exceeds `MAX_TASKS`, extras are
    /// dropped with a warning. Rewrites the storage only when the
    /// loaded set was actually filtered, truncated, or clamped — so
    /// clean loads keep mtime stable.
    pub(crate) async fn load_persisted(&self) -> Result<usize, PersistError> {
        // Lock order: tasks → active.
        let mut guard = self.tasks.write().await;
        let (storage, session_id) = {
            let active = self.active.lock().await;
            let Some(binding) = active.as_ref() else {
                return Ok(0);
            };
            let Some(sid) = binding.session_id.as_ref() else {
                return Ok(0);
            };
            (binding.storage.clone(), sid.clone())
        };
        let persisted = match storage.load(&session_id).await {
            Ok(p) => p,
            Err(e) => {
                self.store_disabled.store(true, Ordering::Release);
                tracing::error!(
                    error = %e,
                    "durable cron load failed; scheduler will refuse to persist until restart"
                );
                return Err(e);
            }
        };
        let loaded_count = persisted.len();
        let now = self.clock.now();
        let mut rehydrated: Vec<ScheduledTask> = Vec::with_capacity(persisted.len());
        let mut clamped_any = false;
        for p in persisted {
            let Ok(mut task) = p.into_task(now) else {
                tracing::warn!("dropping persisted task with unparsable cron");
                continue;
            };
            if !task.recurring {
                let fired = task.last_fired.is_some();
                let next = task.cron.next_after(&task.created_at);
                let missed = next.is_none_or(|t| t <= now);
                if fired || missed {
                    continue;
                }
            } else {
                let reference = task.last_fired.unwrap_or(task.created_at);
                if task.cron.next_after(&reference).is_some_and(|t| t <= now) {
                    task.last_fired = Some(now);
                    clamped_any = true;
                }
            }
            rehydrated.push(task);
        }
        let truncated = if rehydrated.len() > MAX_TASKS {
            tracing::warn!(
                on_disk = rehydrated.len(),
                cap = MAX_TASKS,
                "durable cron file exceeds MAX_TASKS; dropping overflow"
            );
            rehydrated.truncate(MAX_TASKS);
            true
        } else {
            false
        };

        debug_assert!(
            guard.is_empty(),
            "load_persisted assumes empty scheduler; found {} tasks",
            guard.len()
        );
        let before_dedup = rehydrated.len();
        rehydrated.retain(|r| !guard.iter().any(|t| t.id == r.id));
        debug_assert_eq!(
            rehydrated.len(),
            before_dedup,
            "on-disk durable set contained duplicate ids — likely a bug in a prior writer"
        );
        let count = rehydrated.len();
        guard.extend(rehydrated);

        if truncated || clamped_any || count != loaded_count {
            self.persist_locked(&guard).await;
        }
        Ok(count)
    }
}
