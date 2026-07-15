//! Uplink bootstrap — connects a Hub to a MetaHub cluster.
//!
//! Establishes bidirectional connection, starts reverse request handler
//! and heartbeat timer.

use tokio::sync::Mutex;
use tracing::info;

use std::sync::Arc;

use loopal_agent_hub::Hub;

/// Connect this Hub to a MetaHub cluster (bidirectional).
pub async fn connect_to_meta_hub(
    hub: &Arc<Mutex<Hub>>,
    meta_addr: &str,
    hub_name: Option<&str>,
) -> anyhow::Result<()> {
    let token = std::env::var(loopal_protocol::META_HUB_TOKEN_ENV)
        .map_err(|_| anyhow::anyhow!("LOOPAL_META_HUB_TOKEN env var required for --join-hub"))?;

    let name = hub_name
        .map(String::from)
        .unwrap_or_else(|| format!("hub-{}", &uuid::Uuid::new_v4().to_string()[..8]));

    info!(addr = %meta_addr, hub_name = %name, "connecting to MetaHub");
    loopal_agent_hub::uplink_connection::connect(hub, meta_addr, &token, &name)
        .await
        .map_err(anyhow::Error::msg)?;

    info!(hub_name = %name, "joined MetaHub cluster (bidirectional)");
    Ok(())
}
