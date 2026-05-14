use std::sync::atomic::Ordering;

use crate::persistence::{PersistError, durable_snapshot};
use crate::save_worker::{SaveMessage, SaveRequest};
use crate::scheduler::CronScheduler;

impl CronScheduler {
    /// Flush old session's durable subset to its storage, then load the
    /// new session's set. Returns the number of tasks loaded.
    ///
    /// Lock order: tasks → active (see `scheduler.rs` module docs).
    ///
    /// Flush failure does NOT abort: resume is already committed at the
    /// message-history layer, and bouncing here would leave the user
    /// with no cron at all. The save worker emits the error
    /// asynchronously; `dirty` is set so the next tick retries the
    /// (new) session's state.
    ///
    /// `store_disabled` is reset before loading so a previous session's
    /// quarantine state doesn't block the new session's I/O.
    ///
    /// On `Err`: the new session is already bound (`active.session_id`
    /// set, in-memory tasks cleared) and `store_disabled` is latched by
    /// `load_persisted`. Subsequent `add`/`remove` succeed in memory
    /// but do not reach the store — preventing clobbering of the
    /// unreadable on-disk file. Calling `switch_session` again with a
    /// different id resets the latch.
    pub async fn switch_session(&self, new_id: &str) -> Result<usize, PersistError> {
        let mut guard = self.tasks.write().await;
        let flush_target = {
            let mut active = self.active.lock().await;
            let Some(binding) = active.as_mut() else {
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
            let req = SaveRequest {
                storage,
                session_id: old_id,
                snapshot,
                dirty: self.dirty.clone(),
                store_disabled: self.store_disabled.clone(),
            };
            match self.save_tx.try_send(SaveMessage::Save(req)) {
                Ok(()) => self.dirty.store(false, Ordering::Release),
                Err(_) => {
                    tracing::error!("flush on session switch could not enqueue; dirty set");
                    self.dirty.store(true, Ordering::Release);
                }
            }
        }

        self.store_disabled.store(false, Ordering::Release);
        self.dirty.store(false, Ordering::Release);

        guard.clear();
        drop(guard);

        let loaded = self.load_persisted().await?;

        // Notify after load so subscribers see the new session's set,
        // not the empty intermediate state.
        self.notify_change();
        Ok(loaded)
    }
}
