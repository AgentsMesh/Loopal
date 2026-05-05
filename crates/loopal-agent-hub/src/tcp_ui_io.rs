//! TCP UI client IO loop.
//!
//! Handles UI clients (TUI / ACP) that connect via TCP rather than via
//! the in-process `UiSession::connect`. Differences from the local path:
//!
//! - Subscribes to `Hub.ui.event_broadcaster` and forwards each event
//!   to the client as an `agent/event` notification (the local path
//!   exposes the receiver directly).
//! - Registers in `UiDispatcher.clients` so the Hub can route `view/*`
//!   requests and `view/resync_required` notifications to this client.
//! - Reuses the same hub/* and view/* dispatch as `ui_session` —
//!   protocol-level behavior is identical to the local path.

use std::sync::Arc;

use tokio::sync::{Mutex, broadcast::error::RecvError, mpsc};
use tracing::{debug, info, warn};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;

use crate::dispatch::dispatch_hub_request;
use crate::hub::Hub;

/// Spawn the per-TCP-UI-client tasks.
///
/// 1. Register the client connection in `UiDispatcher`.
/// 2. Spawn a forward task: subscribe broadcast → push `agent/event` notifications.
/// 3. Spawn the IO loop that handles incoming `hub/*` and `view/*` requests.
pub fn start_tcp_ui_io(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection>,
    rx: mpsc::Receiver<Incoming>,
) {
    let n = name.to_string();
    let hub_for_io = hub.clone();
    let conn_for_forward = conn.clone();
    tokio::spawn(async move {
        let event_rx = {
            let mut h = hub.lock().await;
            h.ui.register_client(&n, conn.clone());
            h.ui.subscribe_events()
        };
        let conn_io = conn.clone();
        let hub_io = hub_for_io.clone();
        let n_io = n.clone();

        let forward = tokio::spawn(forward_events(n.clone(), event_rx, conn_for_forward));
        tcp_ui_io_loop(hub_io, conn_io, rx, n_io).await;
        forward.abort();
        hub.lock().await.ui.unregister_client(&n);
        info!(client = %n, "TCP UI client disconnected");
    });
}

async fn forward_events(
    client: String,
    mut event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    conn: Arc<Connection>,
) {
    loop {
        match event_rx.recv().await {
            Ok(event) => {
                let Ok(payload) = serde_json::to_value(&event) else {
                    continue;
                };
                if conn
                    .send_notification(methods::AGENT_EVENT.name, payload)
                    .await
                    .is_err()
                {
                    debug!(client = %client, "TCP UI client connection closed; stop forwarding");
                    return;
                }
            }
            Err(RecvError::Lagged(n)) => {
                warn!(client = %client, lagged = n, "TCP UI forward lagged; signaling resync");
                let _ = conn
                    .send_notification(methods::VIEW_RESYNC_REQUIRED.name, serde_json::json!({}))
                    .await;
            }
            Err(RecvError::Closed) => return,
        }
    }
}

async fn tcp_ui_io_loop(
    hub: Arc<Mutex<Hub>>,
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Incoming>,
    name: String,
) {
    info!(client = %name, "TCP UI client IO loop started");
    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Request { id, method, params } => {
                let result = if method == methods::VIEW_SNAPSHOT.name {
                    crate::view_router::handle_snapshot(&hub, params).await
                } else if method.starts_with("hub/") {
                    dispatch_hub_request(&hub, &method, params, name.clone()).await
                } else {
                    Err(format!(
                        "UI clients only support hub/* and view/snapshot, got: {method}"
                    ))
                };
                match result {
                    Ok(value) => {
                        let _ = conn.respond(id, value).await;
                    }
                    Err(e) => {
                        let _ = conn
                            .respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, &e)
                            .await;
                    }
                }
            }
            Incoming::Notification { .. } => {}
        }
    }
    info!(client = %name, "TCP UI client IO loop ended");
}
