use std::sync::Arc;

use loopal_config::McpSharing;
use tracing::info;

use super::HubMcpService;
use super::factory::{build_local_provider, load_servers_by_sharing};

impl HubMcpService {
    pub async fn on_agent_attach(
        &self,
        agent_name: String,
        cwd: std::path::PathBuf,
        _parent_name: Option<String>,
    ) {
        let canonical = cwd.canonicalize().unwrap_or(cwd);
        self.ensure_hub_singleton_for(&canonical).await;
        self.provision_per_agent(&agent_name, &canonical).await;
        self.provision_spawn_tree_if_root_owner(&agent_name, &canonical)
            .await;
        info!(
            agent = %agent_name,
            cwd = %canonical.display(),
            "Hub MCP: on_agent_attach complete"
        );
    }

    pub async fn on_agent_detach(&self, agent_name: &str, was_root: bool) {
        // ADR L1: per-agent dropped on detach.
        if self.per_agent.write().await.remove(agent_name).is_some() {
            info!(agent = %agent_name, "Hub MCP: per-agent instance stopped");
        }

        // Spawn-tree instance is owned by root only; caller passes was_root
        // explicitly so detach is not coupled to SpawnRegistry's current
        // state (the registry may have been unregistered concurrently).
        if was_root && self.spawn_tree.write().await.remove(agent_name).is_some() {
            info!(
                root = %agent_name,
                "Hub MCP: spawn-tree instance stopped (root detached)"
            );
        }
    }

    async fn ensure_hub_singleton_for(&self, cwd: &std::path::Path) {
        let _ = self.provider_for(cwd).await;
    }

    async fn provision_per_agent(&self, agent_name: &str, cwd: &std::path::Path) {
        let servers = load_servers_by_sharing(cwd, McpSharing::PerAgent);
        if servers.is_empty() {
            return;
        }
        let local = build_local_provider(self.secret_client.as_ref(), cwd, servers).await;
        self.per_agent
            .write()
            .await
            .insert(agent_name.to_string(), Arc::new(local));
    }

    async fn provision_spawn_tree_if_root_owner(
        &self,
        agent_name: &str,
        cwd: &std::path::Path,
    ) {
        let Some(root_name) = self.root_of(agent_name) else {
            return;
        };
        if self.spawn_tree.read().await.contains_key(&root_name) {
            return;
        }
        let servers = load_servers_by_sharing(cwd, McpSharing::SpawnTree);
        if servers.is_empty() {
            return;
        }
        let local = build_local_provider(self.secret_client.as_ref(), cwd, servers).await;
        self.spawn_tree
            .write()
            .await
            .insert(root_name, Arc::new(local));
    }

    pub(super) async fn build_hub_singleton(
        &self,
        cwd: &std::path::Path,
    ) -> loopal_mcp::LocalMcpProvider {
        let servers = load_servers_by_sharing(cwd, McpSharing::HubSingleton);
        build_local_provider(self.secret_client.as_ref(), cwd, servers).await
    }
}
