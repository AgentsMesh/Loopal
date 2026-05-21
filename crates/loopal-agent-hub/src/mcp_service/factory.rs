use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopal_config::{McpServerConfig, McpSharing};

pub(super) fn load_servers_by_sharing(
    cwd: &Path,
    sharing: McpSharing,
) -> indexmap::IndexMap<String, McpServerConfig> {
    let config = match loopal_config::load_config(cwd) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(cwd = %cwd.display(), error = %e, "Hub MCP: load_config failed");
            return indexmap::IndexMap::new();
        }
    };
    config
        .settings
        .mcp_servers
        .into_iter()
        .filter(|(_, cfg)| cfg.sharing() == sharing)
        .collect()
}

pub(super) async fn build_local_provider(
    secret_client: Option<&Arc<dyn loopal_secret_client::SecretClient>>,
    cwd: &Path,
    servers: indexmap::IndexMap<String, McpServerConfig>,
) -> loopal_mcp::LocalMcpProvider {
    let manager = Arc::new(tokio::sync::RwLock::new(loopal_mcp::McpManager::new()));
    if let Some(c) = secret_client {
        manager.write().await.set_secret_client(c.clone());
    }
    let local = loopal_mcp::LocalMcpProvider::new(manager);
    if servers.is_empty() {
        return local;
    }
    let with_isolation: indexmap::IndexMap<_, _> = servers
        .into_iter()
        .map(|(name, cfg)| {
            let isolated = super::cwd_isolation::inject(&name, cfg, cwd);
            (name, isolated)
        })
        .collect();
    local.spawn_background(with_isolation);
    local
}

pub(super) fn canonical_or_self(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}
