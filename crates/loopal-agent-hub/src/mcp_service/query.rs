use std::sync::Arc;

use loopal_mcp::McpProvider;

use super::HubMcpService;
use super::factory::canonical_or_self;

impl HubMcpService {
    /// Hub-singleton lazy lookup. Creates instance on first call per cwd.
    pub async fn provider_for(&self, cwd: &std::path::Path) -> Arc<dyn McpProvider> {
        let canonical = canonical_or_self(cwd);
        if let Some(p) = self.hub_singleton.read().await.get(&canonical) {
            return p.clone() as Arc<dyn McpProvider>;
        }
        let mut w = self.hub_singleton.write().await;
        if let Some(p) = w.get(&canonical) {
            return p.clone() as Arc<dyn McpProvider>;
        }
        let local = Arc::new(self.build_hub_singleton(&canonical).await);
        w.insert(canonical, local.clone());
        local as Arc<dyn McpProvider>
    }

    pub async fn local_provider(
        &self,
        cwd: &std::path::Path,
    ) -> Option<Arc<loopal_mcp::LocalMcpProvider>> {
        let canonical = cwd.canonicalize().ok()?;
        self.hub_singleton.read().await.get(&canonical).cloned()
    }

    /// Collect every (server, ToolDefinition) visible to `agent_name`,
    /// applying the same priority used by `provider_for_call`:
    /// per-agent > spawn-tree > hub-singleton. Tools whose `(server)` is
    /// already produced by a higher-priority provider are skipped so the
    /// listing matches what an actual call would dispatch to.
    pub async fn list_tools_for(
        &self,
        agent_name: &str,
        cwd: &std::path::Path,
    ) -> Vec<(String, loopal_tool_api::ToolDefinition)> {
        let mut out: Vec<(String, loopal_tool_api::ToolDefinition)> = Vec::new();
        let mut claimed_servers: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        if let Some(p) = self.per_agent.read().await.get(agent_name) {
            for (server, def) in p.list_tools().await {
                claimed_servers.insert(server.clone());
                out.push((server, def));
            }
        }
        if let Some(r) = self.root_of(agent_name)
            && let Some(p) = self.spawn_tree.read().await.get(&r)
        {
            for (server, def) in p.list_tools().await {
                if claimed_servers.insert(server.clone()) {
                    out.push((server, def));
                }
            }
        }
        let canonical = canonical_or_self(cwd);
        if let Some(p) = self.hub_singleton.read().await.get(&canonical) {
            for (server, def) in p.list_tools().await {
                if claimed_servers.insert(server.clone()) {
                    out.push((server, def));
                }
            }
        }
        out
    }

    /// Resolve which provider owns `server` for `agent_name`, in priority
    /// per-agent → spawn-tree → hub-singleton.
    pub async fn provider_for_call(
        &self,
        agent_name: &str,
        cwd: &std::path::Path,
        server: &str,
    ) -> Option<Arc<dyn McpProvider>> {
        if let Some(p) = self.per_agent.read().await.get(agent_name)
            && p.has_server(server).await
        {
            return Some(p.clone() as Arc<dyn McpProvider>);
        }
        if let Some(r) = self.root_of(agent_name)
            && let Some(p) = self.spawn_tree.read().await.get(&r)
            && p.has_server(server).await
        {
            return Some(p.clone() as Arc<dyn McpProvider>);
        }
        let canonical = cwd.canonicalize().ok()?;
        if let Some(p) = self.hub_singleton.read().await.get(&canonical)
            && p.has_server(server).await
        {
            return Some(p.clone() as Arc<dyn McpProvider>);
        }
        None
    }
}
