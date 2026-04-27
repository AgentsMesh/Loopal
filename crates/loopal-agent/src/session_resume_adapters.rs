//! Adapters that fan out a `SessionResumeHook` notification to the
//! per-session state owned by an agent process.
//!
//! Each adapter wraps a single resource (cron scheduler, task store) so
//! that `agent_setup` can register them independently. The runtime's
//! `handle_resume_session` invokes them in turn after swapping the
//! agent's session id.
//!
//! Failures inside an adapter surface as `SessionResumeError` and are
//! aggregated by the runtime into a single `SessionResumeWarnings`
//! event — the resume itself never aborts, but the user sees which
//! resource failed to follow.

use std::sync::Arc;

use async_trait::async_trait;
use loopal_runtime::{SessionResumeError, SessionResumeHook};
use loopal_scheduler::CronScheduler;

use crate::task_store::TaskStore;

/// Re-binds a [`CronScheduler`] to the new session id on resume.
pub struct CronResumeAdapter {
    scheduler: Arc<CronScheduler>,
}

impl CronResumeAdapter {
    pub fn new(scheduler: Arc<CronScheduler>) -> Self {
        Self { scheduler }
    }
}

#[async_trait]
impl SessionResumeHook for CronResumeAdapter {
    async fn on_session_changed(&self, new_session_id: &str) -> Result<(), SessionResumeError> {
        self.scheduler
            .switch_session(new_session_id)
            .await
            .map(|_| ())
            .map_err(|e| SessionResumeError::new("cron", e.to_string()))
    }
}

/// Re-binds a [`TaskStore`] to the new session id on resume.
pub struct TaskResumeAdapter {
    task_store: Arc<TaskStore>,
}

impl TaskResumeAdapter {
    pub fn new(task_store: Arc<TaskStore>) -> Self {
        Self { task_store }
    }
}

#[async_trait]
impl SessionResumeHook for TaskResumeAdapter {
    async fn on_session_changed(&self, new_session_id: &str) -> Result<(), SessionResumeError> {
        self.task_store
            .switch_session(new_session_id)
            .await
            .map(|_| ())
            .map_err(|e| SessionResumeError::new("task", e.to_string()))
    }
}
