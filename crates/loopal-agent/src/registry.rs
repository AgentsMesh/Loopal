use std::collections::HashMap;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::types::AgentId;

/// Handle for a spawned sub-agent, stored in the registry.
pub struct AgentHandle {
    pub id: AgentId,
    pub name: String,
    pub agent_type: String,
    pub cancel_token: CancellationToken,
    pub join_handle: JoinHandle<()>,
}

/// Registry of all live sub-agents, keyed by name.
pub struct AgentRegistry {
    agents: HashMap<String, AgentHandle>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Register a new sub-agent handle.
    pub fn register(&mut self, handle: AgentHandle) {
        self.agents.insert(handle.name.clone(), handle);
    }

    /// Look up a sub-agent by name.
    pub fn get(&self, name: &str) -> Option<&AgentHandle> {
        self.agents.get(name)
    }

    /// Remove a sub-agent from the registry (e.g. after completion).
    pub fn remove(&mut self, name: &str) -> Option<AgentHandle> {
        self.agents.remove(name)
    }

    /// Iterate over all registered agents.
    pub fn iter(&self) -> impl Iterator<Item = &AgentHandle> {
        self.agents.values()
    }

    /// Number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
