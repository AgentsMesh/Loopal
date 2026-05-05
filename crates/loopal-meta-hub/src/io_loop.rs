//! MetaHub IO loop — processes requests from a single connected Sub-Hub.

use std::sync::Arc;

use tokio::sync::Mutex;

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;

use crate::dispatch::dispatch_meta_request;
use crate::meta_hub::MetaHub;

/// IO loop for a single Sub-Hub connection.
///
/// Processes `meta/*` requests and `agent/event` notifications.
/// Runs until the Sub-Hub disconnects.
pub async fn meta_hub_io_loop(
    meta_hub: Arc<Mutex<MetaHub>>,
    conn: Arc<loopal_ipc::connection::Connection>,
    mut rx: tokio::sync::mpsc::Receiver<Incoming>,
    hub_name: String,
) {
    tracing::info!(hub = %hub_name, "Sub-Hub IO loop started");

    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Request { id, method, params } => {
                match dispatch_meta_request(&meta_hub, &method, params, hub_name.clone()).await {
                    Ok(result) => {
                        let _ = conn.respond(id, result).await;
                    }
                    Err(e) => {
                        tracing::warn!(hub = %hub_name, %method, error = %e, "request failed");
                        let _ = conn
                            .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &e)
                            .await;
                    }
                }
            }
            Incoming::Notification { method, params } => {
                if method == methods::META_HEARTBEAT.name {
                    let agent_count = params["agent_count"].as_u64().unwrap_or(0) as usize;
                    let mut mh = meta_hub.lock().await;
                    let _ = mh.registry.heartbeat(&hub_name, agent_count);
                }
            }
        }
    }

    // Sub-Hub disconnected — cleanup
    tracing::info!(hub = %hub_name, "Sub-Hub IO loop ended, cleaning up");
    let mut mh = meta_hub.lock().await;
    mh.remove_hub(&hub_name);
}
