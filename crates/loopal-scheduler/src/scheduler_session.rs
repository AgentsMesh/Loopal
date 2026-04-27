//! `CronScheduler::switch_session` — atomic active-session swap.
//!
//! `switch_session(new_id)` is the single point where session-scoped
//! storage is re-bound at runtime. It:
//!
//! 1. Acquires `tasks` write + `active` mutex (lock order: tasks → active,
//!    matching [`crate::scheduler_persistence`]).
//! 2. No-op if the binding already names `new_id`.
//! 3. If a previous session was bound, flushes the current in-memory
//!    durable subset to its `cron.json`. Flush failure is logged + sets
//!    `dirty`; the call **does not abort** — resume is already
//!    semantically committed at the message-history layer, and bouncing
//!    here would leave the user with no cron at all.
//! 4. Clears in-memory tasks, resets `store_disabled` (the new session's
//!    file may be perfectly readable even if the old one was quarantined),
//!    sets `active.session_id = Some(new_id)`.
//! 5. Loads the new session's persisted set (filter rules in
//!    [`crate::scheduler_persistence`]).
//!
//! No-op when the scheduler has no storage attached
//! ([`CronScheduler::new`](crate::CronScheduler::new) /
//! [`with_clock`](crate::CronScheduler::with_clock) paths) — those
//! schedulers are in-memory-only by design.

use std::sync::atomic::Ordering;

use crate::persistence::{PersistError, durable_snapshot};
use crate::scheduler::CronScheduler;

impl CronScheduler {
    /// Re-bind the scheduler to a different session, flushing the current
    /// in-memory durable set to the previous session's storage and
    /// loading the new session's persisted tasks.
    ///
    /// Returns the number of tasks loaded for the new session.
    ///
    /// See module-level docs for the algorithm and failure semantics.
    pub async fn switch_session(&self, new_id: &str) -> Result<usize, PersistError> {
        // --- Phase 1: flush old + swap binding (under tasks + active locks).
        let mut guard = self.tasks.write().await;
        let flush_target = {
            let mut active = self.active.lock().await;
            let Some(binding) = active.as_mut() else {
                // No storage attached at all — purely in-memory scheduler.
                return Ok(0);
            };
            if binding.session_id.as_deref() == Some(new_id) {
                return Ok(0);
            }
            let target = binding
                .session_id
                .clone()
                .map(|sid| (binding.storage.clone(), sid));
            binding.session_id = Some(new_id.to_string());
            target
        };

        if let Some((storage, old_id)) = flush_target
            && !self.store_disabled.load(Ordering::Acquire)
        {
            let snapshot = durable_snapshot(&guard);
            if let Err(e) = storage.save_all(&old_id, &snapshot).await {
                tracing::error!(
                    error = %e,
                    old_session = %old_id,
                    "flush on session switch failed; setting dirty for retry"
                );
                self.dirty.store(true, Ordering::Release);
            } else {
                self.dirty.store(false, Ordering::Release);
            }
        }

        // Reset both latches: the new session starts on a clean slate.
        //
        // - `store_disabled`: the previous session's quarantine state
        //   must not block the new session's storage I/O.
        // - `dirty`: any "old session needs retry" signal is meaningless
        //   now that the in-memory task list belongs to a different
        //   session. Phase 2 (`load_persisted`) sets `dirty` itself if
        //   the new session's load triggers a follow-up `persist_locked`
        //   that fails, so the new session's retry semantics still work.
        self.store_disabled.store(false, Ordering::Release);
        self.dirty.store(false, Ordering::Release);

        // Drop in-memory tasks before phase 2's empty-list precondition.
        guard.clear();
        drop(guard);

        // --- Phase 2: load the new session's persisted state. The load
        // path re-acquires `tasks` write + `active` mutex internally;
        // since we've dropped both above, no nested locking conflict.
        let loaded = self.load_persisted().await?;

        // Notify subscribers exactly once per swap, *after* the new
        // session's tasks are visible. Doing it after `load_persisted`
        // means the bridge re-snapshots and sees the new session's job
        // set, not an empty intermediate state.
        self.notify_change();
        Ok(loaded)
    }
}
