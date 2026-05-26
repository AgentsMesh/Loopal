mod cwd_isolation;
mod factory;
mod lifecycle;
mod query;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use loopal_mcp::LocalMcpProvider;
use loopal_secret_client::SecretClient;
use tokio::sync::RwLock;

use crate::spawn_registry::SpawnRegistry;

pub struct HubMcpService {
    pub(super) hub_singleton: RwLock<HashMap<PathBuf, Arc<LocalMcpProvider>>>,
    pub(super) per_agent: RwLock<HashMap<String, Arc<LocalMcpProvider>>>,
    pub(super) spawn_tree: RwLock<HashMap<String, Arc<LocalMcpProvider>>>,
    pub(super) secret_client: Option<Arc<dyn SecretClient>>,
    pub(super) spawn_registry: Option<Arc<SpawnRegistry>>,
}

impl HubMcpService {
    pub fn new() -> Self {
        Self {
            hub_singleton: RwLock::new(HashMap::new()),
            per_agent: RwLock::new(HashMap::new()),
            spawn_tree: RwLock::new(HashMap::new()),
            secret_client: None,
            spawn_registry: None,
        }
    }

    pub fn with_secret_client(mut self, client: Arc<dyn SecretClient>) -> Self {
        self.secret_client = Some(client);
        self
    }

    pub fn with_spawn_registry(mut self, registry: Arc<SpawnRegistry>) -> Self {
        self.spawn_registry = Some(registry);
        self
    }

    /// Delegate to SpawnRegistry. `HubMcpService` holds no parent-chain
    /// state of its own — the registry is the single writer (ADR §7).
    pub(super) fn root_of(&self, agent_name: &str) -> Option<String> {
        self.spawn_registry
            .as_ref()
            .and_then(|r| r.root_of(agent_name))
    }
}

impl Default for HubMcpService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_spawn_tree;
