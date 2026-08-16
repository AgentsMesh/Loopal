use loopal_protocol::{AgentEventPayload, AgentStateSnapshot, WorkflowRunId};

use crate::conversation::AgentConversation;
use crate::delta::ViewSnapshot;
use crate::mutators::mutate;
use crate::state::SessionViewState;

pub struct ViewStateReducer {
    state: SessionViewState,
    rev: u64,
}

/// Per-run projection discontinuity. The rejected event is not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRevisionGap {
    pub run_id: WorkflowRunId,
    pub expected_revision: u64,
    pub actual_revision: u64,
}

/// Detailed reducer result for callers that can trigger snapshot resync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewStateApplyOutcome {
    Applied { revision: u64 },
    NoOp,
    ResyncRequired(WorkflowRevisionGap),
}

impl ViewStateReducer {
    pub fn new(agent_name: impl Into<String>) -> Self {
        Self {
            state: SessionViewState::empty(agent_name),
            rev: 0,
        }
    }

    pub fn from_snapshot(agent_name: impl Into<String>, snapshot: AgentStateSnapshot) -> Self {
        Self {
            state: SessionViewState::from_snapshot(agent_name, snapshot),
            rev: 1,
        }
    }

    pub fn rev(&self) -> u64 {
        self.rev
    }

    pub fn state(&self) -> &SessionViewState {
        &self.state
    }

    pub fn with_conversation_mut<R>(&mut self, f: impl FnOnce(&mut AgentConversation) -> R) -> R {
        f(&mut self.state.agent.conversation)
    }

    pub fn with_view_mut<R>(&mut self, f: impl FnOnce(&mut crate::state::AgentView) -> R) -> R {
        f(&mut self.state.agent)
    }

    pub fn snapshot(&self) -> ViewSnapshot {
        ViewSnapshot {
            rev: self.rev,
            state: self.state.clone(),
        }
    }

    pub fn reset_to(&mut self, snapshot: ViewSnapshot) {
        self.state = snapshot.state;
        self.rev = snapshot.rev;
    }

    pub fn apply(&mut self, event: AgentEventPayload) -> Option<u64> {
        match self.apply_checked(event) {
            ViewStateApplyOutcome::Applied { revision } => Some(revision),
            ViewStateApplyOutcome::NoOp | ViewStateApplyOutcome::ResyncRequired(_) => None,
        }
    }

    pub fn apply_with_rev(&mut self, event: AgentEventPayload, target_rev: u64) -> Option<u64> {
        match self.apply_with_rev_checked(event, target_rev) {
            ViewStateApplyOutcome::Applied { revision } => Some(revision),
            ViewStateApplyOutcome::NoOp | ViewStateApplyOutcome::ResyncRequired(_) => None,
        }
    }

    pub fn apply_checked(&mut self, event: AgentEventPayload) -> ViewStateApplyOutcome {
        let next_revision = self.rev.saturating_add(1);
        self.apply_checked_inner(event, next_revision)
    }

    pub fn apply_with_rev_checked(
        &mut self,
        event: AgentEventPayload,
        target_rev: u64,
    ) -> ViewStateApplyOutcome {
        self.apply_checked_inner(event, target_rev)
    }

    fn apply_checked_inner(
        &mut self,
        event: AgentEventPayload,
        target_rev: u64,
    ) -> ViewStateApplyOutcome {
        let effect = mutate(&mut self.state, &event);
        if let crate::mutators::MutationEffect::WorkflowRevisionGap(gap) = &effect {
            return ViewStateApplyOutcome::ResyncRequired(gap.clone());
        }
        if !effect.changed() {
            return ViewStateApplyOutcome::NoOp;
        }
        if effect.requires_turn_end_reconcile() {
            crate::conversation::tool_result_handler::handle_turn_end_reconcile(
                &mut self.state.agent.conversation,
            );
        }
        self.rev = target_rev;
        ViewStateApplyOutcome::Applied { revision: self.rev }
    }
}
