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

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;

use crate::dispatch::build_hub_dispatcher;
use crate::hub::Hub;
use crate::ui_request_loop::ui_client_io_loop;

/// Spawn the per-TCP-UI-client tasks.
///
/// 1. Register the client connection in `UiDispatcher`.
/// 2. Spawn a forward task: subscribe broadcast → push `agent/event` notifications.
/// 3. Spawn the IO loop that handles incoming `hub/*` and `view/*` requests.
pub fn start_tcp_ui_io(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: mpsc::Receiver<Incoming>,
) {
    let n = name.to_string();
    let hub_for_io = hub.clone();
    let conn_for_forward = conn.clone();
    let dispatcher = Arc::new(build_hub_dispatcher(hub.clone()));
    tokio::spawn(async move {
        let event_rx = {
            let mut h = hub.lock().await;
            h.ui.register_client(&n, conn.clone());
            h.ui.subscribe_events()
        };
        let service_rx = hub
            .lock()
            .await
            .workspace
            .as_ref()
            .map(|service| service.subscribe());
        let conn_io = conn.clone();
        let hub_io = hub_for_io.clone();
        let n_io = n.clone();

        let forward = tokio::spawn(forward_events(n.clone(), event_rx, conn_for_forward));
        let service_forward =
            service_rx.map(|rx| tokio::spawn(forward_service_events(n.clone(), rx, conn.clone())));
        ui_client_io_loop(hub_io, dispatcher, conn_io, rx, n_io).await;
        forward.abort();
        if let Some(task) = service_forward {
            task.abort();
        }
        hub.lock().await.ui.unregister_client(&n);
        info!(client = %n, "TCP UI client disconnected");
    });
}

async fn forward_service_events(
    client: String,
    mut event_rx: tokio::sync::broadcast::Receiver<loopal_workspace::ServiceNotification>,
    conn: Arc<Connection<Listening>>,
) {
    loop {
        match event_rx.recv().await {
            Ok(event) => {
                if conn
                    .send_notification(event.method, event.params)
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(RecvError::Lagged(n)) => {
                warn!(client = %client, lagged = n, "workspace event forward lagged");
                if send_service_lag(&conn, n).await.is_err() {
                    return;
                }
            }
            Err(RecvError::Closed) => return,
        }
    }
}

async fn send_service_lag(
    conn: &Connection<Listening>,
    dropped: u64,
) -> Result<(), loopal_ipc::rpc_error::RpcError> {
    conn.send_notification(
        methods::WORKSPACE_RESYNC_REQUIRED.name,
        serde_json::json!({
            "workspaceId": loopal_workspace::LOCAL_WORKSPACE_ID,
            "reason": "event_lag",
            "droppedEvents": dropped,
        }),
    )
    .await
}

async fn forward_events(
    client: String,
    mut event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    conn: Arc<Connection<Listening>>,
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
