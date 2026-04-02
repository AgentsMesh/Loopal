//! Uplink bootstrap — connects a Hub to a MetaHub cluster.
//!
//! Establishes bidirectional connection, starts reverse request handler
//! and heartbeat timer.

use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::info;

use loopal_agent_hub::{Hub, HubUplink};
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::tcp::TcpTransport;

/// Connect this Hub to a MetaHub cluster (bidirectional).
pub async fn connect_to_meta_hub(
    hub: &Arc<Mutex<Hub>>,
    meta_addr: &str,
    hub_name: Option<&str>,
) -> anyhow::Result<()> {
    let token = std::env::var("LOOPAL_META_HUB_TOKEN")
        .map_err(|_| anyhow::anyhow!("LOOPAL_META_HUB_TOKEN env var required for --join-hub"))?;

    let name = hub_name
        .map(String::from)
        .unwrap_or_else(|| format!("hub-{}", &uuid::Uuid::new_v4().to_string()[..8]));

    info!(addr = %meta_addr, hub_name = %name, "connecting to MetaHub");

    let stream = TcpStream::connect(meta_addr).await?;
    let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let rx = conn.start();

    let resp = conn
        .send_request(
            methods::META_REGISTER.name,
            serde_json::json!({"name": name, "token": token, "capabilities": []}),
        )
        .await
        .map_err(|e| anyhow::anyhow!("meta/register failed: {e}"))?;

    if resp.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        anyhow::bail!("MetaHub rejected registration: {resp}");
    }

    let uplink = Arc::new(HubUplink::new(conn.clone(), name.clone()));
    hub.lock().await.uplink = Some(uplink.clone());

    // Reverse request handler (shared implementation from uplink module)
    let reverse_hub = hub.clone();
    let reverse_conn = conn;
    let reverse_name = name.clone();
    tokio::spawn(async move {
        loopal_agent_hub::uplink::handle_reverse_requests(
            reverse_hub,
            reverse_conn,
            rx,
            reverse_name,
        )
        .await;
    });

    // Heartbeat timer (15s interval)
    let heartbeat_hub = hub.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            let count = heartbeat_hub.lock().await.registry.agent_count();
            if let Err(e) = uplink.heartbeat(count).await {
                tracing::warn!(error = %e, "heartbeat to MetaHub failed");
                break;
            }
        }
    });

    info!(hub_name = %name, "joined MetaHub cluster (bidirectional)");
    Ok(())
}
