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

    /// Bypasses `apply()` and does NOT bump `rev` — UI-local mutations
    /// (cursor moves, dialog dismissal) that must not be broadcast.
    pub fn with_conversation_mut<R>(&mut self, f: impl FnOnce(&mut AgentConversation) -> R) -> R {
        f(&mut self.state.agent.conversation)
    }

    /// Bypasses `apply()` and does NOT bump `rev`.
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

    /// Apply an event. Returns `Some(new_rev)` when state changed,
    /// `None` if the event is non-observable (no rev bump).
    pub fn apply(&mut self, event: AgentEventPayload) -> Option<u64> {
        if mutate(&mut self.state, &event) {
            self.rev += 1;
            Some(self.rev)
        } else {
            None
        }
    }

    /// Apply an event and stamp `rev` to `target_rev` instead of
    /// self-incrementing. UI clients use this to follow the
    /// authoritative rev sequence assigned by the Hub-side reducer,
    /// so dropped events (e.g. `Lagged`) don't cause silent drift.
    pub fn apply_with_rev(&mut self, event: AgentEventPayload, target_rev: u64) -> Option<u64> {
        if mutate(&mut self.state, &event) {
            self.rev = target_rev;
            Some(self.rev)
        } else {
            None
        }
    }
}
