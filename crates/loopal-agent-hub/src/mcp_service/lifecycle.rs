use std::sync::Arc;

use loopal_config::McpSharing;
use tracing::info;

use super::HubMcpService;
use super::factory::{build_local_provider_with_redaction_seed, load_servers_by_sharing};
use crate::types::AgentExecutionRef;

impl HubMcpService {
    pub(crate) async fn on_agent_attach(
        &self,
        execution: AgentExecutionRef,
        cwd: std::path::PathBuf,
    ) {
        let canonical = cwd.canonicalize().unwrap_or(cwd);
        self.ensure_hub_singleton_for(&canonical).await;
        self.provision_per_agent(&execution, &canonical).await;
        self.provision_spawn_tree_if_root_owner(&execution, &canonical)
            .await;
        info!(
            agent = %execution.address,
            generation = execution.connection_generation,
            cwd = %canonical.display(),
            "Hub MCP: exact Agent attach complete"
        );
    }

    pub(crate) async fn on_agent_detach(&self, execution: &AgentExecutionRef) {
        if self.per_agent.write().await.remove(execution).is_some() {
            info!(agent = %execution.address, "Hub MCP: per-agent instance stopped");
        }
        if self.spawn_tree.write().await.remove(execution).is_some() {
            info!(root = %execution.address, "Hub MCP: spawn-tree instance stopped");
        }
    }

    async fn ensure_hub_singleton_for(&self, cwd: &std::path::Path) {
        let _ = self.provider_for(cwd).await;
    }

    async fn provision_per_agent(&self, execution: &AgentExecutionRef, cwd: &std::path::Path) {
        let servers = load_servers_by_sharing(cwd, McpSharing::PerAgent);
        if servers.is_empty() {
            return;
        }
        let local = build_local_provider_with_redaction_seed(
            self.vault_service.as_ref(),
            cwd,
            servers,
            self.final_sink_redaction_seed.clone(),
        )
        .await;
        if !self.topology_owns(execution) {
            return;
        }
        self.per_agent
            .write()
            .await
            .insert(execution.clone(), Arc::new(local));
    }

    async fn provision_spawn_tree_if_root_owner(
        &self,
        execution: &AgentExecutionRef,
        cwd: &std::path::Path,
    ) {
        let Some(root) = self.root_of(execution) else {
            return;
        };
        if self.spawn_tree.read().await.contains_key(&root) {
            return;
        }
        let servers = load_servers_by_sharing(cwd, McpSharing::SpawnTree);
        if servers.is_empty() {
            return;
        }
        let local = build_local_provider_with_redaction_seed(
            self.vault_service.as_ref(),
            cwd,
            servers,
            self.final_sink_redaction_seed.clone(),
        )
        .await;
        if self.root_of(execution).as_ref() != Some(&root) {
            return;
        }
        self.spawn_tree.write().await.insert(root, Arc::new(local));
    }

    fn topology_owns(&self, execution: &AgentExecutionRef) -> bool {
        self.spawn_registry
            .as_ref()
            .and_then(|registry| registry.cwd_for(execution))
            .is_some()
    }

    pub(super) async fn build_hub_singleton(
        &self,
        cwd: &std::path::Path,
    ) -> loopal_mcp::LocalMcpProvider {
        let servers = load_servers_by_sharing(cwd, McpSharing::HubSingleton);
        build_local_provider_with_redaction_seed(
            self.vault_service.as_ref(),
            cwd,
            servers,
            self.final_sink_redaction_seed.clone(),
        )
        .await
    }
}
