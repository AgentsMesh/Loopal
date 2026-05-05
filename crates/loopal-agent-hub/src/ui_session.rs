//! UI Session — client-side handle for a UI client connected to Hub.
//!
//! Encapsulates all the wiring needed to connect a UI client
//! to the Hub: connection + event subscription. Created via
//! `UiSession::connect()` — one line replaces all bootstrap glue.

use std::sync::Arc;

use tokio::sync::{Mutex, broadcast, mpsc};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::AgentEvent;

use crate::dispatch::dispatch_hub_request;
use crate::hub::Hub;
use crate::hub_ui_client::HubClient;

/// A connected UI client session.
pub struct UiSession {
    pub client: Arc<HubClient>,
    pub event_rx: broadcast::Receiver<AgentEvent>,
}

impl UiSession {
    pub async fn connect(hub: Arc<Mutex<Hub>>, name: &str) -> Self {
        let (client_transport, server_transport) = loopal_ipc::duplex_pair();

        let client_conn = Arc::new(Connection::new(client_transport));
        let server_conn = Arc::new(Connection::new(server_transport));

        let client_rx = client_conn.start();
        let server_rx = server_conn.start();

        {
            let mut h = hub.lock().await;
            h.ui.register_client(name, server_conn.clone());
        }

        let event_rx = hub.lock().await.ui.subscribe_events();

        let hub_for_io = hub.clone();
        let io_name = name.to_string();
        tokio::spawn(async move {
            ui_client_io_loop(hub_for_io.clone(), server_conn, server_rx, io_name.clone()).await;
            hub_for_io.lock().await.ui.unregister_client(&io_name);
        });

        // Drain client_rx — UiSession doesn't surface IPC incoming because
        // events arrive via broadcast and Hub no longer relays IPC requests
        // through the duplex.
        tokio::spawn(drain_incoming(client_rx));

        let client = Arc::new(HubClient::new(client_conn));

        Self { client, event_rx }
    }
}

async fn drain_incoming(mut rx: mpsc::Receiver<Incoming>) {
    while rx.recv().await.is_some() {}
}

async fn ui_client_io_loop(
    hub: Arc<Mutex<Hub>>,
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Incoming>,
    name: String,
) {
    use loopal_ipc::protocol::methods;
    tracing::info!(client = %name, "UI client IO loop started");
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
    tracing::info!(client = %name, "UI client IO loop ended");
}
