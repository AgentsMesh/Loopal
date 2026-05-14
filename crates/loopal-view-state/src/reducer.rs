use loopal_protocol::{AgentEventPayload, AgentStateSnapshot};

use crate::conversation::AgentConversation;
use crate::delta::ViewSnapshot;
use crate::mutators::mutate;
use crate::state::SessionViewState;

pub struct ViewStateReducer {
    state: SessionViewState,
    rev: u64,
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
        self.apply_inner(event)?;
        self.rev += 1;
        Some(self.rev)
    }

    pub fn apply_with_rev(&mut self, event: AgentEventPayload, target_rev: u64) -> Option<u64> {
        self.apply_inner(event)?;
        self.rev = target_rev;
        Some(self.rev)
    }

    fn apply_inner(&mut self, event: AgentEventPayload) -> Option<()> {
        let effect = mutate(&mut self.state, &event);
        if !effect.changed() {
            return None;
        }
        if effect.requires_turn_end_reconcile() {
            crate::conversation::tool_result_handler::handle_turn_end_reconcile(
                &mut self.state.agent.conversation,
            );
        }
        Some(())
    }
}
