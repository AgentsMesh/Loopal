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

impl AgentLifecycle {
    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self, Self::Finished | Self::Failed(_))
    }

    pub fn state(&self) -> &'static str {
        match self {
            Self::Spawning => "spawning",
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Failed(_) => "failed",
        }
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
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
}
