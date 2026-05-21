//! UI Session — client-side handle for a UI client connected to Hub.
//!
//! Encapsulates all the wiring needed to connect a UI client
//! to the Hub: connection + event subscription. Created via
//! `UiSession::connect()` — one line replaces all bootstrap glue.

use std::sync::Arc;

use tokio::sync::{Mutex, broadcast, mpsc};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::AgentEvent;

use crate::dispatch::build_hub_dispatcher;
use crate::hub::Hub;
use crate::hub_ui_client::HubClient;
use crate::ui_request_loop::ui_client_io_loop;

/// A connected UI client session.
pub struct UiSession {
    pub client: Arc<HubClient>,
    pub event_rx: broadcast::Receiver<AgentEvent>,
}

impl UiSession {
    pub async fn connect(hub: Arc<Mutex<Hub>>, name: &str) -> Self {
        let (client_transport, server_transport) = loopal_ipc::duplex_pair();

        let (client_conn, client_rx) = Connection::new(client_transport).into_listening();
        let (server_conn, server_rx) = Connection::new(server_transport).into_listening();

        let event_rx = {
            let mut h = hub.lock().await;
            h.ui.register_client(name, server_conn.clone());
            h.ui.subscribe_events()
        };

        let hub_for_io = hub.clone();
        let io_name = name.to_string();
        let dispatcher = Arc::new(build_hub_dispatcher(hub.clone()));
        tokio::spawn(async move {
            ui_client_io_loop(
                hub_for_io.clone(),
                dispatcher,
                server_conn,
                server_rx,
                io_name.clone(),
            )
            .await;
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
