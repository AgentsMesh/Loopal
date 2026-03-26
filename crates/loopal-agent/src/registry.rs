use std::collections::HashMap;
use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use loopal_agent_client::AgentProcess;

use crate::types::AgentId;

/// Handle for a spawned sub-agent, stored in the registry.
pub struct AgentHandle {
    pub id: AgentId,
    pub name: String,
    pub agent_type: String,
    pub cancel_token: CancellationToken,
    pub join_handle: JoinHandle<()>,
    /// The child process handle — used for lifecycle management.
    pub process: Arc<tokio::sync::Mutex<Option<AgentProcess>>>,
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

    /// Shutdown all registered agents (graceful process termination).
    pub async fn shutdown_all(&mut self) {
        for (_, handle) in self.agents.drain() {
            handle.cancel_token.cancel();
            if let Some(proc) = handle.process.lock().await.take() {
                let _ = proc.shutdown().await;
            }
        }
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
