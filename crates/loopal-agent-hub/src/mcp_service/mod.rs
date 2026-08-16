mod cwd_isolation;
mod factory;
mod lifecycle;
mod query;
mod reconnect;
mod vault_resolver;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use loopal_mcp::LocalMcpProvider;
use loopal_output_guard::FinalSinkRedactionSeed;
use tokio::sync::RwLock;

use crate::spawn_registry::SpawnRegistry;
use crate::types::AgentExecutionRef;

pub struct HubMcpService {
    pub(super) hub_singleton: RwLock<HashMap<PathBuf, Arc<LocalMcpProvider>>>,
    pub(super) per_agent: RwLock<HashMap<AgentExecutionRef, Arc<LocalMcpProvider>>>,
    pub(super) spawn_tree: RwLock<HashMap<AgentExecutionRef, Arc<LocalMcpProvider>>>,
    pub(super) vault_service: Option<Arc<loopal_hub_vault::HubVaultService>>,
    pub(super) spawn_registry: Option<Arc<SpawnRegistry>>,
    pub(super) final_sink_redaction_seed: FinalSinkRedactionSeed,
}

impl HubMcpService {
    pub fn new() -> Self {
        Self::new_with_redaction_seed(FinalSinkRedactionSeed::new())
    }

    pub fn new_with_redaction_seed(final_sink_redaction_seed: FinalSinkRedactionSeed) -> Self {
        Self {
            hub_singleton: RwLock::new(HashMap::new()),
            per_agent: RwLock::new(HashMap::new()),
            spawn_tree: RwLock::new(HashMap::new()),
            vault_service: None,
            spawn_registry: None,
            final_sink_redaction_seed,
        }
    }

    pub fn with_vault_service(mut self, vault: Arc<loopal_hub_vault::HubVaultService>) -> Self {
        self.vault_service = Some(vault);
        self
    }

    pub fn with_spawn_registry(mut self, registry: Arc<SpawnRegistry>) -> Self {
        self.spawn_registry = Some(registry);
        self
    }

    /// Delegate to SpawnRegistry. `HubMcpService` holds no parent-chain
    /// state of its own — the registry is the single writer (ADR §7).
    pub(super) fn root_of(&self, execution: &AgentExecutionRef) -> Option<AgentExecutionRef> {
        self.spawn_registry
            .as_ref()
            .and_then(|registry| registry.root_execution(execution))
    }
}

impl Default for HubMcpService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod construction_tests;
#[cfg(test)]
mod factory_tests;
#[cfg(test)]
mod query_tests;
#[cfg(test)]
pub(crate) mod test_vault;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_spawn_tree;
