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
            append_tools(
                &mut out,
                &mut claimed_servers,
                p.list_tools(loopal_mcp::HUB_RPC_BUDGET).await,
            );
        }
        if let Some(r) = self.root_of(agent_name)
            && let Some(p) = self.spawn_tree.read().await.get(&r)
        {
            append_tools(
                &mut out,
                &mut claimed_servers,
                p.list_tools(loopal_mcp::HUB_RPC_BUDGET).await,
            );
        }
        let canonical = canonical_or_self(cwd);
        if let Some(p) = self.hub_singleton.read().await.get(&canonical) {
            append_tools(
                &mut out,
                &mut claimed_servers,
                p.list_tools(loopal_mcp::HUB_RPC_BUDGET).await,
            );
        }
        out
    }

    pub async fn snapshots_for(
        &self,
        agent_name: &str,
        cwd: &std::path::Path,
    ) -> Vec<loopal_mcp::McpConnectionSnapshot> {
        let mut out = Vec::new();
        let mut claimed = std::collections::HashSet::new();
        if let Some(provider) = self.per_agent.read().await.get(agent_name) {
            append_snapshots(&mut out, &mut claimed, provider.as_ref()).await;
        }
        if let Some(root) = self.root_of(agent_name)
            && let Some(provider) = self.spawn_tree.read().await.get(&root)
        {
            append_snapshots(&mut out, &mut claimed, provider.as_ref()).await;
        }
        let canonical = canonical_or_self(cwd);
        if let Some(provider) = self.hub_singleton.read().await.get(&canonical) {
            append_snapshots(&mut out, &mut claimed, provider.as_ref()).await;
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

fn append_tools(
    out: &mut Vec<(String, loopal_tool_api::ToolDefinition)>,
    claimed: &mut std::collections::HashSet<String>,
    tools: Vec<(String, loopal_tool_api::ToolDefinition)>,
) {
    let available: std::collections::HashSet<_> =
        tools.iter().map(|(server, _)| server.clone()).collect();
    out.extend(
        tools
            .into_iter()
            .filter(|(server, _)| !claimed.contains(server)),
    );
    claimed.extend(available);
}

async fn append_snapshots(
    out: &mut Vec<loopal_mcp::McpConnectionSnapshot>,
    claimed: &mut std::collections::HashSet<String>,
    provider: &dyn McpProvider,
) {
    for snapshot in provider.snapshot(loopal_mcp::HUB_RPC_BUDGET).await {
        if claimed.insert(snapshot.name.clone()) {
            out.push(snapshot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::append_tools;

    fn tool(name: &str) -> loopal_tool_api::ToolDefinition {
        loopal_tool_api::ToolDefinition {
            name: name.into(),
            description: String::new(),
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    #[test]
    fn provider_priority_keeps_every_tool_from_a_claimed_server() {
        let mut out = Vec::new();
        let mut claimed = std::collections::HashSet::new();
        append_tools(
            &mut out,
            &mut claimed,
            vec![
                ("server".into(), tool("one")),
                ("server".into(), tool("two")),
            ],
        );
        append_tools(
            &mut out,
            &mut claimed,
            vec![
                ("server".into(), tool("shadowed")),
                ("other".into(), tool("three")),
            ],
        );
        let names: Vec<_> = out.into_iter().map(|(_, tool)| tool.name).collect();
        assert_eq!(names, ["one", "two", "three"]);
    }
}
