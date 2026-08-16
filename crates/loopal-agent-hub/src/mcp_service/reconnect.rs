use std::sync::Arc;

use super::HubMcpService;
use super::factory::canonical_or_self;
use crate::types::AgentExecutionRef;

impl HubMcpService {
    pub(crate) async fn reconnect_for(
        &self,
        execution: &AgentExecutionRef,
        cwd: &std::path::Path,
        server: &str,
    ) -> Option<bool> {
        let provider = self.provider_owning(execution, cwd, server).await?;
        let registry = self.spawn_registry.clone()?;
        let execution = execution.clone();
        Some(
            provider
                .try_reconnect_guarded(server, move |commit| {
                    registry.while_current(&execution, commit);
                })
                .await,
        )
    }

    async fn provider_owning(
        &self,
        execution: &AgentExecutionRef,
        cwd: &std::path::Path,
        server: &str,
    ) -> Option<Arc<loopal_mcp::LocalMcpProvider>> {
        let per_agent = self.per_agent.read().await.get(execution).cloned();
        if let Some(provider) = per_agent
            && provider.owns_server(server).await
        {
            return Some(provider);
        }
        let tree = match self.root_of(execution) {
            Some(root) => self.spawn_tree.read().await.get(&root).cloned(),
            None => None,
        };
        if let Some(provider) = tree
            && provider.owns_server(server).await
        {
            return Some(provider);
        }
        let canonical = canonical_or_self(cwd);
        let singleton = self.hub_singleton.read().await.get(&canonical).cloned();
        match singleton {
            Some(provider) if provider.owns_server(server).await => Some(provider),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "reconnect_tests.rs"]
mod tests;
