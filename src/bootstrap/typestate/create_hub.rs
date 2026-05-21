use std::path::Path;
use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};
use tracing::info;

use loopal_agent_hub::Hub;

use super::states::HubBuilt;

impl HubBuilt {
    pub async fn new(cwd: &Path, config: &loopal_config::ResolvedConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(256);
        let hub = Arc::new(Mutex::new(Hub::with_cwd(event_tx, cwd.to_path_buf())));
        hub.lock().await.max_total_agents = config.settings.harness.agent_max_total;

        init_vault(&hub).await;

        HubBuilt { hub, event_rx }
    }
}

async fn init_vault(hub: &Arc<Mutex<Hub>>) {
    match loopal_hub_vault::HubVaultService::with_noop_audit() {
        Ok(vault) => {
            hub.lock().await.set_vault_service(Arc::new(vault));
            info!("Hub vault service initialized");
        }
        Err(e) => {
            tracing::warn!(error = %e, "Hub vault service unavailable; hub/secret/* will fail");
        }
    }
}
