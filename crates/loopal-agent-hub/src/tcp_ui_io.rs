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

use tokio::sync::{Mutex, mpsc};
use tracing::info;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::UiCapabilities;

use crate::dispatch::build_hub_dispatcher;
use crate::hub::Hub;
use crate::ui_request_loop::ui_client_io_loop;

#[path = "tcp_ui_io/forward.rs"]
mod forward;
use forward::{forward_events, forward_service_events};

/// Spawn the per-TCP-UI-client tasks.
///
/// 1. Register the client connection in `UiDispatcher`.
/// 2. Spawn a forward task: subscribe broadcast → push `agent/event` notifications.
/// 3. Spawn the IO loop that handles incoming `hub/*` and `view/*` requests.
pub async fn start_tcp_ui_io(
    hub: Arc<Mutex<Hub>>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: mpsc::Receiver<Incoming>,
    capabilities: UiCapabilities,
    lease_id: String,
) {
    let n = name.to_string();
    let lease = lease_id;
    let hub_for_io = hub.clone();
    let conn_for_forward = conn.clone();
    let shutdown_conn = conn.clone();
    let dispatcher = Arc::new(build_hub_dispatcher(hub.clone()));
    let (event_rx, resync_rx) = {
        let mut h = hub.lock().await;
        h.ui.register_client_with_lease(&lease, &n, conn.clone(), capabilities);
        (h.ui.subscribe_events(), h.ui.subscribe_resync())
    };
    let service_rx = hub
        .lock()
        .await
        .workspace
        .as_ref()
        .map(|service| service.subscribe());
    tokio::spawn(async move {
        let conn_io = conn.clone();
        let hub_io = hub_for_io.clone();
        let n_io = lease.clone();

        let mut forward = tokio::spawn(forward_events(
            n.clone(),
            event_rx,
            resync_rx,
            conn_for_forward,
        ));
        let mut service_forward = tokio::spawn({
            let n = n.clone();
            let conn = conn.clone();
            async move {
                match service_rx {
                    Some(rx) => forward_service_events(n, rx, conn).await,
                    None => std::future::pending().await,
                }
            }
        });
        let mut input = tokio::spawn(ui_client_io_loop(hub_io, dispatcher, conn_io, rx, n_io));
        enum Finished {
            Input,
            Events,
            Service,
        }
        let finished = tokio::select! {
            _ = &mut input => Finished::Input,
            _ = &mut forward => Finished::Events,
            _ = &mut service_forward => Finished::Service,
        };
        match finished {
            Finished::Input => {
                forward.abort();
                service_forward.abort();
                let _ = tokio::join!(forward, service_forward);
            }
            Finished::Events => {
                input.abort();
                service_forward.abort();
                let _ = tokio::join!(input, service_forward);
            }
            Finished::Service => {
                input.abort();
                forward.abort();
                let _ = tokio::join!(input, forward);
            }
        }
        hub.lock().await.ui.unregister_client(&lease);
        crate::pending_relay::cleanup_without_capable_ui(&hub).await;
        let _ =
            tokio::time::timeout(std::time::Duration::from_secs(2), shutdown_conn.close()).await;
        info!(client = %n, "TCP UI client disconnected");
    });
}

#[cfg(test)]
#[path = "tcp_ui_io_tests.rs"]
mod tests;
