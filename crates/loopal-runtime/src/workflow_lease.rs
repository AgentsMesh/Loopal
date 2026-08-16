use std::collections::HashSet;
use std::sync::Mutex;

use loopal_protocol::WorkflowRunId;
use loopal_turn::{Turn, TurnTrigger};

#[derive(Default)]
pub struct WorkflowLeaseTracker {
    state: Mutex<LeaseState>,
}

#[derive(Default)]
struct LeaseState {
    outstanding: HashSet<WorkflowRunId>,
    completed: HashSet<WorkflowRunId>,
}

impl WorkflowLeaseTracker {
    pub fn recovered(turns: &[Turn], outstanding: Vec<WorkflowRunId>) -> Self {
        let completed = turns.iter().filter_map(|turn| match &turn.trigger {
            TurnTrigger::WorkflowResult { run_id, .. } => Some(WorkflowRunId::new(run_id.clone())),
            _ => None,
        });
        let tracker = Self::default();
        {
            let mut state = tracker.state.lock().expect("workflow lease lock poisoned");
            state.completed.extend(completed);
            for run_id in outstanding {
                if !state.completed.contains(&run_id) {
                    state.outstanding.insert(run_id);
                }
            }
        }
        tracker
    }

    /// Hold a process-lifetime lease only after the Hub confirms the start.
    pub fn track(&self, run_id: WorkflowRunId) -> bool {
        let mut state = self.state.lock().expect("workflow lease lock poisoned");
        if state.completed.contains(&run_id) {
            return false;
        }
        state.outstanding.insert(run_id)
    }

    /// Record a tombstone even if terminal delivery raced ahead of start ACK.
    pub fn complete(&self, run_id: &WorkflowRunId) -> bool {
        let mut state = self.state.lock().expect("workflow lease lock poisoned");
        let removed = state.outstanding.remove(run_id);
        state.completed.insert(run_id.clone());
        removed
    }

    pub fn has_outstanding(&self) -> bool {
        !self
            .state
            .lock()
            .expect("workflow lease lock poisoned")
            .outstanding
            .is_empty()
    }

    #[cfg(test)]
    fn outstanding(&self) -> HashSet<WorkflowRunId> {
        self.state
            .lock()
            .expect("workflow lease lock poisoned")
            .outstanding
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use loopal_turn::{Turn, TurnOutcome, TurnTrigger};

    use super::*;

    #[test]
    fn leases_are_exact_idempotent_and_completed_ids_stay_closed() {
        let tracker = WorkflowLeaseTracker::default();
        let first = WorkflowRunId::new("wrun_first");
        let second = WorkflowRunId::new("wrun_second");

        assert!(tracker.track(first.clone()));
        assert!(!tracker.track(first.clone()));
        assert!(tracker.track(second.clone()));
        assert!(tracker.complete(&first));
        assert!(!tracker.track(first.clone()));
        assert_eq!(tracker.outstanding(), HashSet::from([second]));
    }

    #[test]
    fn recovery_prefers_durable_result_turns_over_unacked_journals() {
        let completed = WorkflowRunId::new("wrun_completed");
        let pending = WorkflowRunId::new("wrun_pending");
        let mut turn = Turn::new(TurnTrigger::WorkflowResult {
            session_id: "session".into(),
            run_id: completed.to_string(),
            terminal_revision: 3,
            payload_digest: "digest".into(),
            state: "succeeded".into(),
            content: "done".into(),
        });
        turn.outcome = TurnOutcome::Complete;

        let tracker =
            WorkflowLeaseTracker::recovered(&[turn], vec![completed.clone(), pending.clone()]);

        assert_eq!(tracker.outstanding(), HashSet::from([pending]));
        assert!(!tracker.track(completed));
    }
}
