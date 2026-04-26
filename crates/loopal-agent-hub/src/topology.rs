//! Agent topology — tracks parent/child relationships and lifecycle state.

use std::time::Instant;

use loopal_protocol::QualifiedAddress;

/// Lifecycle state of an agent managed by the Hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentLifecycle {
    /// Process is being spawned (fork + IPC init).
    Spawning,
    /// Agent loop is running.
    Running,
    /// Agent completed successfully, output available.
    Finished,
    /// Agent terminated with an error.
    Failed(String),
}

/// Metadata and relationship info for a managed agent.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    /// Who spawned this agent. `None` for root.
    /// Local parents have `hub.is_empty()`; remote (cross-hub) parents
    /// carry a hub path stamped at spawn time.
    pub parent: Option<QualifiedAddress>,
    /// Locally-visible children (bare names; cross-hub children appear
    /// here as shadow entries by their final-hop name).
    pub children: Vec<String>,
    pub lifecycle: AgentLifecycle,
    pub model: Option<String>,
    pub spawned_at: Instant,
}

impl AgentInfo {
    pub fn new(name: &str, parent: Option<QualifiedAddress>, model: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            parent,
            children: Vec::new(),
            lifecycle: AgentLifecycle::Spawning,
            model: model.map(String::from),
            spawned_at: Instant::now(),
        }
    }

    /// Collect all descendant names (depth-first).
    pub(crate) fn descendants(
        &self,
        agents: &std::collections::HashMap<String, super::types::ManagedAgent>,
    ) -> Vec<String> {
        let mut result = Vec::new();
        let mut stack: Vec<String> = self.children.clone();
        while let Some(name) = stack.pop() {
            result.push(name.clone());
            if let Some(agent) = agents.get(&name) {
                stack.extend(agent.info.children.iter().cloned());
            }
        }
        result
    }
}
