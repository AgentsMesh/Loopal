//! Convenience snapshot accessors for panel rendering.
//!
//! Panel render functions take owned slices because they're shared
//! between TUI and ACP renderers. Each call clones the current view;
//! for the small per-agent panels this is cheap and avoids holding
//! the read lock across rendering.

use loopal_protocol::{BgTaskSnapshot, CronJobSnapshot, TaskSnapshot};

use super::ViewClient;

impl ViewClient {
    pub fn task_snapshots(&self) -> Vec<TaskSnapshot> {
        self.inner
            .read()
            .expect("view client lock poisoned")
            .state()
            .tasks
            .clone()
    }

    pub fn cron_snapshots(&self) -> Vec<CronJobSnapshot> {
        self.inner
            .read()
            .expect("view client lock poisoned")
            .state()
            .crons
            .clone()
    }

    pub fn bg_snapshots(&self) -> Vec<BgTaskSnapshot> {
        let guard = self.inner.read().expect("view client lock poisoned");
        guard
            .state()
            .bg_tasks
            .values()
            .map(|v| BgTaskSnapshot {
                id: v.id.clone(),
                description: v.description.clone(),
                status: v.status,
                exit_code: v.exit_code,
            })
            .collect()
    }
}
