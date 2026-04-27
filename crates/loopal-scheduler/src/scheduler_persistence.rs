//! `CronScheduler` methods that touch the [`SessionScopedCronStorage`].
//!
//! Split out so `scheduler.rs` stays focused on the in-memory CRUD path
//! and persistence integration has room to grow (loading, retry, schema
//! evolution).
//!
//! Both `persist_locked` and `load_persisted` consult
//! [`crate::scheduler::ActiveBinding`] under the `active` mutex to fetch
//! the current `(session_id, storage)`. Lock order is always
//! `tasks` → `active` (see [`crate::scheduler`] module docs).

use std::sync::atomic::Ordering;

use crate::persistence::{PersistError, durable_snapshot};
use crate::scheduler::{CronScheduler, MAX_TASKS};
use crate::task::ScheduledTask;

impl CronScheduler {
    /// Write the current durable-task snapshot to the attached storage.
    ///
    /// Callers must hold the `tasks` write lock so concurrent mutations
    /// don't interleave saves. A `None` binding (no storage attached, or
    /// session not yet bound) is a no-op. Failure is logged and sets the
    /// `dirty` flag so the next tick or mutation retries.
    ///
    /// If `store_disabled` is latched (e.g. quarantine failed at load
    /// time), this becomes a silent no-op — preventing clobbering an
    /// unrecognized on-disk file with an empty in-memory set.
    pub(crate) async fn persist_locked(&self, tasks: &[ScheduledTask]) {
        if self.store_disabled.load(Ordering::Acquire) {
            return;
        }
        let active = self.active.lock().await;
        let Some(binding) = active.as_ref() else {
            return;
        };
        let Some(session_id) = binding.session_id.as_ref() else {
            return;
        };
        let snapshot = durable_snapshot(tasks);
        match binding.storage.save_all(session_id, &snapshot).await {
            Ok(()) => {
                self.dirty.store(false, Ordering::Release);
            }
            Err(e) => {
                tracing::error!(error = %e, "cron durable save failed; will retry");
                self.dirty.store(true, Ordering::Release);
            }
        }
    }

    /// Load persisted tasks from the storage into memory.
    ///
    /// Internal helper invoked by [`switch_session`](crate::CronScheduler::switch_session).
    /// External callers should use `switch_session(id)` instead — it
    /// performs flush + clear + load atomically.
    ///
    /// **Preconditions**: assumes the in-memory task list is empty —
    /// calling this on a populated scheduler panics in debug builds
    /// and silently prefers existing IDs in release builds.
    ///
    /// **Filter rules** (drop-on-load, no catch-up):
    /// - expired (`is_expired(now)`) — past the 3-day lifetime (durable
    ///   tasks are exempt)
    /// - one-shot already fired (`last_fired.is_some()`)
    /// - one-shot whose next fire time has passed (`next_after(created_at) ≤ now`)
    ///
    /// **Normalization**: for recurring tasks whose scheduled reference
    /// (`last_fired` or `created_at`) is in the past — which would
    /// otherwise cause an immediate "catch-up" fire on the next tick —
    /// the `last_fired` field is clamped to `now`.
    ///
    /// **Capacity**: if the on-disk set exceeds [`MAX_TASKS`], the
    /// extras are dropped with a warning so subsequent `add` calls can
    /// still succeed.
    ///
    /// **Side effect**: rewrites the storage only when the loaded set
    /// was actually filtered / truncated / clamped, to keep mtime
    /// stable on clean loads.
    pub(crate) async fn load_persisted(&self) -> Result<usize, PersistError> {
        // Lock order: tasks → active. Acquire `tasks` first to keep
        // ordering symmetric with `persist_locked` callers (add/remove)
        // and `switch_session`.
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
            if task.is_expired(&now) {
                continue;
            }
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
