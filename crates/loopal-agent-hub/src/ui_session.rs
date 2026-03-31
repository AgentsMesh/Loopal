//! UI Session — client-side handle for a UI client connected to Hub.
//!
//! Encapsulates all the wiring needed to connect a UI client
//! to the Hub: connection, event subscription, permission relay.
//! Created via `UiSession::connect()` — one line replaces all bootstrap glue.

use std::sync::Arc;

use tokio::sync::{Mutex, broadcast, mpsc};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::AgentEvent;

use crate::dispatch::dispatch_hub_request;
use crate::hub::Hub;
use crate::hub_ui_client::HubClient;

/// A connected UI client session — holds everything a UI client needs.
pub struct UiSession {
    /// Typed client for sending messages/control/interrupts to Hub.
    pub client: Arc<HubClient>,
    /// Agent events from Hub broadcast.
    pub event_rx: broadcast::Receiver<AgentEvent>,
    /// Incoming relay requests (permission/question) from Hub.
    pub relay_rx: mpsc::Receiver<Incoming>,
}

impl UiSession {
    /// Connect to Hub as a UI client. Handles all wiring:
    /// 1. Create duplex pair
    /// 2. Register in UiDispatcher (NOT AgentRegistry)
    /// 3. Subscribe to event broadcast
    /// 4. Start IO loop for hub/* requests
    pub async fn connect(hub: Arc<Mutex<Hub>>, name: &str) -> Self {
        // Create duplex pair: client_side (for HubClient) ↔ server_side (Hub handles)
        let (client_transport, server_transport) = loopal_ipc::duplex_pair();

        let client_conn = Arc::new(Connection::new(client_transport));
        let server_conn = Arc::new(Connection::new(server_transport));

        let client_rx = client_conn.start();
        let server_rx = server_conn.start();

        // Register in UiDispatcher with server-side connection
        {
            let mut h = hub.lock().await;
            h.ui.register_client(name, server_conn.clone());
        }

        // Subscribe to events
        let event_rx = hub.lock().await.ui.subscribe_events();

        // Start IO loop for this UI client (handles hub/* requests)
        let hub_for_io = hub.clone();
        let io_name = name.to_string();
        tokio::spawn(async move {
            ui_client_io_loop(hub_for_io, server_conn, server_rx, io_name).await;
        });

        let client = Arc::new(HubClient::new(client_conn));

        Self {
            client,
            event_rx,
            relay_rx: client_rx,
        }
    }
}

/// IO loop for a UI client's server-side connection.
///
/// Simpler than `agent_io_loop`: only handles `hub/*` requests.
/// No event forwarding, no completion tracking, no permission relay.
async fn ui_client_io_loop(
    hub: Arc<Mutex<Hub>>,
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Incoming>,
    name: String,
) {
    tracing::info!(client = %name, "UI client IO loop started");
    while let Some(msg) = rx.recv().await {
        match msg {
            Incoming::Request { id, method, params } => {
                if method.starts_with("hub/") {
                    match dispatch_hub_request(&hub, &method, params, name.clone()).await {
                        Ok(result) => {
                            let _ = conn.respond(id, result).await;
                        }
                        Err(e) => {
                            let _ = conn
                                .respond_error(id, loopal_ipc::jsonrpc::METHOD_NOT_FOUND, &e)
                                .await;
                        }
                    }
                } else {
                    let _ = conn
                        .respond_error(
                            id,
                            loopal_ipc::jsonrpc::METHOD_NOT_FOUND,
                            &format!("UI clients only support hub/* methods, got: {method}"),
                        )
                        .await;
                }
            }
            Incoming::Notification { .. } => {}
        }
    }
    tracing::info!(client = %name, "UI client IO loop ended");
}
