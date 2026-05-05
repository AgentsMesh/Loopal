//! Test-only data injection helpers.
//!
//! Wrap `apply_event` with matching `AgentEventPayload` variants so
//! tests can populate the local view without going through the agent
//! loop. **Append semantics** (existing data is preserved) so tests
//! that previously did `app.x_snapshots.push(...)` keep working with
//! `app.view_client.inject_x_for_test(vec![...])`. Annotated
//! `#[doc(hidden)]` because production code should drive ViewClient
//! via real events.

use loopal_protocol::{
    AgentEvent, AgentEventPayload, BgTaskSnapshot, BgTaskStatus, CronJobSnapshot, TaskSnapshot,
};

use super::ViewClient;

impl ViewClient {
    #[doc(hidden)]
    pub fn inject_tasks_for_test(&self, tasks: Vec<TaskSnapshot>) {
        let mut current = self.task_snapshots();
        current.extend(tasks);
        self.apply_event(&AgentEvent::root(AgentEventPayload::TasksChanged {
            tasks: current,
        }));
    }

    #[doc(hidden)]
    pub fn inject_crons_for_test(&self, crons: Vec<CronJobSnapshot>) {
        let mut current = self.cron_snapshots();
        current.extend(crons);
        self.apply_event(&AgentEvent::root(AgentEventPayload::CronsChanged {
            crons: current,
        }));
    }

    #[doc(hidden)]
    pub fn inject_bg_for_test(&self, items: Vec<BgTaskSnapshot>) {
        for item in items {
            self.apply_event(&AgentEvent::root(AgentEventPayload::BgTaskSpawned {
                id: item.id.clone(),
                description: item.description.clone(),
            }));
            if !matches!(item.status, BgTaskStatus::Running) {
                self.apply_event(&AgentEvent::root(AgentEventPayload::BgTaskCompleted {
                    id: item.id,
                    status: item.status,
                    exit_code: item.exit_code,
                    output: String::new(),
                }));
            }
        }
    }
}
