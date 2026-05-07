use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn continuation_still_consistent(&self) -> bool {
        let goal_id = match self.last_continuation_goal_id.as_ref() {
            Some(id) => id,
            None => return true,
        };
        let session = match self.params.goal_session.as_ref() {
            Some(s) => s,
            None => return false,
        };
        let goal = match session.snapshot().await {
            Ok(Some(g)) => g,
            _ => return false,
        };
        if &goal.goal_id != goal_id {
            return false;
        }
        goal.status.participates_in_continuation()
    }
}
