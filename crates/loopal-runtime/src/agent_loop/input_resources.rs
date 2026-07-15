use loopal_tool_background::ops::bg_stop;
use tracing::{info, warn};

use super::input_control::ControlOutcome;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn handle_bg_task_kill(&self, id: String) -> ControlOutcome {
        info!(id = %id, "killing bg task via control command");
        let store = self.params.deps.kernel.bg_store().clone();
        let result = bg_stop(&store, &id).await;
        if result.is_error {
            ControlOutcome::rejected(result.content)
        } else {
            ControlOutcome::applied()
        }
    }

    pub(super) async fn handle_cron_delete(&self, id: String) -> ControlOutcome {
        info!(id = %id, "deleting cron via control command");
        let Some(sched) = self.params.scheduler.as_ref() else {
            warn!(id = %id, "scheduler unavailable; cron delete ignored");
            return ControlOutcome::rejected("cron scheduler is unavailable");
        };
        let removed = sched.remove(&id).await;
        if !removed {
            warn!(id = %id, "cron delete: no job found");
            return ControlOutcome::rejected(format!("cron job not found: {id}"));
        }
        ControlOutcome::applied()
    }
}
