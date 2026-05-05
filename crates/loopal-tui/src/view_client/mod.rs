//! TUI-side view client — local replica of one agent's `SessionViewState`.
//!
//! Wraps a `ViewStateReducer`. The TUI feeds incoming `AgentEvent`s
//! (received over the existing `agent/event` broadcast or local mpsc)
//! via `apply_event`; the reducer maintains the `SessionViewState`
//! that panel renderers read. Initial state is seeded from a
//! `view/snapshot` response via `from_snapshot`. `RwLock` (std, not
//! tokio) is used because all access is short — the lock never crosses
//! an `.await`.

mod snapshots;
mod test_helpers;

use std::sync::{Arc, RwLock, RwLockReadGuard};

use loopal_protocol::AgentEvent;
use loopal_view_state::{AgentConversation, SessionViewState, ViewSnapshot, ViewStateReducer};

#[derive(Clone)]
pub struct ViewClient {
    pub(super) agent: String,
    pub(super) inner: Arc<RwLock<ViewStateReducer>>,
}

impl ViewClient {
    /// Construct from an initial `view/snapshot` response.
    pub fn from_snapshot(agent: impl Into<String>, snapshot: ViewSnapshot) -> Self {
        let agent = agent.into();
        let mut reducer = ViewStateReducer::new(&agent);
        reducer.reset_to(snapshot);
        Self {
            agent,
            inner: Arc::new(RwLock::new(reducer)),
        }
    }

    /// Construct an empty client. Used when no snapshot has been pulled
    /// yet — the local replica catches up via subsequent `apply_event`
    /// calls from the live `agent/event` stream.
    pub fn empty(agent: impl Into<String>) -> Self {
        let agent = agent.into();
        Self {
            agent: agent.clone(),
            inner: Arc::new(RwLock::new(ViewStateReducer::new(&agent))),
        }
    }

    pub fn agent(&self) -> &str {
        &self.agent
    }

    pub fn rev(&self) -> u64 {
        self.inner.read().expect("view client lock poisoned").rev()
    }

    /// Read-only access to the local `SessionViewState`. Lock is held
    /// for the duration of the returned guard — release before any
    /// `.await` to avoid deadlocks with concurrent applies.
    pub fn state(&self) -> ViewClientStateGuard<'_> {
        let guard = self.inner.read().expect("view client lock poisoned");
        ViewClientStateGuard { guard }
    }

    pub fn apply_event(&self, event: &AgentEvent) {
        let event_agent = event.agent_name.as_ref().map(|q| q.agent.as_str());
        let matches = match event_agent {
            Some(name) => name == self.agent,
            None => self.agent == "main",
        };
        if !matches {
            return;
        }
        let mut reducer = self.inner.write().expect("view client lock poisoned");
        match event.rev {
            Some(rev) if rev <= reducer.rev() => {}
            Some(rev) => {
                reducer.apply_with_rev(event.payload.clone(), rev);
            }
            None => {
                reducer.apply(event.payload.clone());
            }
        }
    }

    /// Replace inner reducer state with `snapshot`. UI-local conversation
    /// messages (where `ui_local == true`) are preserved across the swap
    /// — the snapshot only carries Hub-tracked rows.
    pub fn reset_to_snapshot(&self, snapshot: ViewSnapshot) {
        let mut reducer = self.inner.write().expect("view client lock poisoned");
        let preserved: Vec<(usize, loopal_view_state::SessionMessage)> = reducer
            .state()
            .agent
            .conversation
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.ui_local)
            .map(|(i, m)| (i, m.clone()))
            .collect();
        reducer.reset_to(snapshot);
        reducer.with_conversation_mut(|conv| {
            for (idx, msg) in preserved {
                let pos = idx.min(conv.messages.len());
                conv.messages.insert(pos, msg);
            }
        });
    }

    /// Mutate this agent's conversation in-place (UI-only mutations like
    /// cursor moves and dialog dismissal). Does not bump `rev`.
    pub fn with_conversation_mut<R>(&self, f: impl FnOnce(&mut AgentConversation) -> R) -> R {
        let mut reducer = self.inner.write().expect("view client lock poisoned");
        reducer.with_conversation_mut(f)
    }

    /// Mutate this agent's `AgentView` directly (e.g., to seed `parent`
    /// at creation time). Does not bump `rev`.
    pub fn with_view_mut<R>(&self, f: impl FnOnce(&mut loopal_view_state::AgentView) -> R) -> R {
        let mut reducer = self.inner.write().expect("view client lock poisoned");
        reducer.with_view_mut(f)
    }
}

/// Read guard over the local `SessionViewState`. Holding this prevents
/// concurrent `apply_event` calls; release promptly.
pub struct ViewClientStateGuard<'a> {
    guard: RwLockReadGuard<'a, ViewStateReducer>,
}

impl ViewClientStateGuard<'_> {
    pub fn state(&self) -> &SessionViewState {
        self.guard.state()
    }

    pub fn rev(&self) -> u64 {
        self.guard.rev()
    }

    pub fn conversation(&self) -> &AgentConversation {
        &self.guard.state().agent.conversation
    }
}
