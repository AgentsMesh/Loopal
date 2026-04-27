//! `TaskStore::switch_session` — atomic active-session swap.
//!
//! Symmetric with [`CronScheduler::switch_session`](loopal_scheduler::CronScheduler::switch_session):
//! returns `Result<usize, io::Error>` (loaded count) on success. Flush
//! failure surfaces as `Err`, but the new session is loaded regardless
//! so observers always see the post-swap state. Load failure on the
//! new session also surfaces as `Err` (does not silently fall back to
//! empty state).

use crate::task_store::TaskStore;

impl TaskStore {
    pub async fn switch_session(&self, new_id: &str) -> std::io::Result<usize> {
        let _persist_guard = self.persist_mutex().lock().await;

        let (old_session, old_tasks) = {
            let inner = self.inner.read().await;
            if inner.active_session_id.as_deref() == Some(new_id) {
                return Ok(inner.tasks.len());
            }
            (inner.active_session_id.clone(), inner.tasks.clone())
        };

        let mut flush_err: Option<std::io::Error> = None;
        if let Some(ref old_id) = old_session
            && let Err(e) = self.storage().save_all(old_id, &old_tasks).await
        {
            tracing::error!(
                error = %e,
                old_session = %old_id,
                "task store flush on session switch failed"
            );
            flush_err = Some(e);
        }

        // Load failure cannot be ignored — return Err but still install
        // empty state so the agent observes a consistent (empty) view.
        let load_result = self.storage().load(new_id).await;
        let (new_tasks, next_id) = match &load_result {
            Ok((t, n)) => (t.clone(), *n),
            Err(_) => (Vec::new(), 1),
        };
        let loaded = new_tasks.len();

        {
            let mut inner = self.inner.write().await;
            inner.tasks = new_tasks;
            inner.next_id = next_id;
            inner.active_session_id = Some(new_id.to_string());
        }

        drop(_persist_guard);
        self.notify_change();

        // Flush error takes priority over load error: flush is the
        // bigger concern (data loss for the previous session); load
        // error is recoverable (caller may re-resume after fix).
        if let Some(e) = flush_err {
            return Err(e);
        }
        load_result.map(|_| loaded)
    }
}
