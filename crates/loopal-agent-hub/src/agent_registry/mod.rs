//! Agent registry — manages agent connections, lifecycle, routing.
//!
//! Contains only agent-related state. UI client management is in `UiDispatcher`.

mod completion;
mod queries;
mod tombstones;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use loopal_ipc::connection::{Connection, Listening};
use loopal_protocol::{AgentEvent, Envelope, QualifiedAddress};

use crate::topology::AgentInfo;
use crate::types::{AgentConnectionState, CompletedAgent, LocalChannels, ManagedAgent};

pub const MAX_COMPLETED_AGENTS: usize = 128;

/// Pure agent registry — no UI client knowledge.
pub struct AgentRegistry {
    pub(crate) agents: HashMap<String, ManagedAgent>,
    pub(crate) event_tx: mpsc::Sender<AgentEvent>,
    pub(crate) completions: HashMap<String, watch::Sender<Option<String>>>,
    pub(crate) completed: HashMap<String, CompletedAgent>,
    pub(crate) completed_order: VecDeque<String>,
    pub(crate) completed_limit: usize,
}

impl AgentRegistry {
    pub fn new(event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            agents: HashMap::new(),
            event_tx,
            completions: HashMap::new(),
            completed: HashMap::new(),
            completed_order: VecDeque::new(),
            completed_limit: MAX_COMPLETED_AGENTS,
        }
    }

    pub fn event_sender(&self) -> mpsc::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    // ── Registration ─────────────────────────────────────────────

    pub fn set_local(&mut self, name: &str, channels: LocalChannels) {
        self.forget_completed(name);
        let view = ManagedAgent::new_view_reducer(name);
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Local(channels),
                info: AgentInfo::new(name, None, None),
                completion_tx: None,
                notify_parent_on_completion: true,
                view,
                output: None,
            },
        );
    }

    pub fn register_connection(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
    ) -> Result<(), String> {
        self.register_connection_with_parent(name, conn, None, None, None)
    }

    pub fn register_connection_with_parent(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
    ) -> Result<(), String> {
        self.register_connection_with_parent_policy(name, conn, parent, model, completion_tx, true)
    }

    pub fn register_connection_with_parent_policy(
        &mut self,
        name: &str,
        conn: Arc<Connection<Listening>>,
        parent: Option<QualifiedAddress>,
        model: Option<&str>,
        completion_tx: Option<mpsc::Sender<Envelope>>,
        notify_parent_on_completion: bool,
    ) -> Result<(), String> {
        if self.agents.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        self.forget_completed(name);
        // Local children are tracked on the parent only when the parent is local.
        if let Some(p) = &parent
            && p.is_local()
            && let Some(pa) = self.agents.get_mut(&p.agent)
        {
            pa.info.children.push(name.to_string());
        }
        let view = ManagedAgent::new_view_reducer(name);
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Connected(conn),
                info: AgentInfo::new(name, parent, model),
                completion_tx,
                notify_parent_on_completion,
                view,
                output: None,
            },
        );
        Ok(())
    }

    pub fn unregister_connection(&mut self, name: &str) {
        self.detach_agent(name);
        self.completions.remove(name);
    }

    /// Register a shadow entry for a remotely-spawned agent.
    ///
    /// Returns Err if `name` is already registered (local or shadow). Callers
    /// should treat this as authoritative — never overwrite an existing entry,
    /// because that would silently destroy an active agent's state.
    pub fn register_shadow(&mut self, name: &str, parent: QualifiedAddress) -> Result<(), String> {
        if self.agents.contains_key(name) {
            return Err(format!("agent '{name}' already registered"));
        }
        self.forget_completed(name);
        let parent_for_children = parent.clone();
        let mut info = AgentInfo::new(name, Some(parent), None);
        info.lifecycle = crate::AgentLifecycle::Running;
        let view = ManagedAgent::new_view_reducer(name);
        self.agents.insert(
            name.to_string(),
            ManagedAgent {
                state: AgentConnectionState::Shadow,
                info,
                completion_tx: None,
                notify_parent_on_completion: true,
                view,
                output: None,
            },
        );
        // Track in parent's children list when parent is local.
        if parent_for_children.is_local()
            && let Some(pa) = self.agents.get_mut(&parent_for_children.agent)
        {
            pa.info.children.push(name.to_string());
        }
        tracing::info!(agent = %name, parent = %parent_for_children,
            "shadow registered for remote agent");
        Ok(())
    }
}
