use std::path::Path;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tracing::info;

use loopal_agent_hub::Hub;
use loopal_vault_api::AuditSink;

use super::states::HubBuilt;

impl HubBuilt {
    pub async fn new(cwd: &Path, config: &loopal_config::ResolvedConfig) -> anyhow::Result<Self> {
        Self::new_with(
            cwd,
            config,
            |config| {
                loopal_secret_runtime::JsonlAuditSink::try_new(
                    config.settings.telemetry.telemetry_dir(),
                )
                .map(|sink| Arc::new(sink) as Arc<dyn AuditSink>)
                .map_err(|error| error.to_string())
            },
            |audit| {
                loopal_hub_vault::HubVaultService::new(audit).map_err(|error| error.to_string())
            },
        )
        .await
    }

    async fn new_with(
        cwd: &Path,
        config: &loopal_config::ResolvedConfig,
        make_audit: impl FnOnce(&loopal_config::ResolvedConfig) -> Result<Arc<dyn AuditSink>, String>,
        make_vault: impl FnOnce(Arc<dyn AuditSink>) -> Result<loopal_hub_vault::HubVaultService, String>,
    ) -> anyhow::Result<Self> {
        let (event_tx, event_rx) = mpsc::channel(256);
        let hub = Arc::new(Mutex::new(Hub::with_cwd(event_tx, cwd.to_path_buf())));
        {
            let mut locked = hub.lock().await;
            locked.max_total_agents = config.settings.harness.agent_max_total;
            locked.max_agent_depth = config.settings.harness.agent_max_depth;
            locked.set_root_spawn_authority(&config.settings);
        }
        init_vault_with(&hub, make_audit(config), make_vault).await?;
        Ok(HubBuilt { hub, event_rx })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum VaultInit {
    VaultUnavailable,
    Ready,
}

async fn init_vault_with(
    hub: &Arc<Mutex<Hub>>,
    audit: Result<Arc<dyn AuditSink>, String>,
    make_vault: impl FnOnce(Arc<dyn AuditSink>) -> Result<loopal_hub_vault::HubVaultService, String>,
) -> anyhow::Result<VaultInit> {
    let audit = match audit {
        Ok(audit) => audit,
        Err(error) => {
            return Err(anyhow::anyhow!(
                "protected audit initialization failed: {error}"
            ));
        }
    };
    hub.lock().await.set_protected_audit(audit.clone());
    match make_vault(audit) {
        Ok(vault) => {
            let vault = Arc::new(vault);
            let mut locked = hub.lock().await;
            let mcp = loopal_agent_hub::HubMcpService::new_with_redaction_seed(
                locked.final_sink_redaction_seed(),
            )
            .with_spawn_registry(locked.spawn_registry.clone())
            .with_vault_service(vault.clone());
            locked.set_vault_service(vault);
            locked.set_mcp_service(Arc::new(mcp));
            info!("Hub vault and MCP secret services initialized");
            Ok(VaultInit::Ready)
        }
        Err(error) => {
            tracing::warn!(%error, "Hub vault service unavailable; hub/secret/* will fail");
            Ok(VaultInit::VaultUnavailable)
        }
    }
}

#[cfg(test)]
#[path = "create_hub_tests.rs"]
mod tests;
