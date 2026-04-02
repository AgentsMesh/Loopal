//! Shared test helpers for cross-hub integration tests.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::AgentEvent;
use serde_json::json;

use loopal_meta_hub::MetaHub;

pub fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Wire a Sub-Hub to MetaHub via in-process duplex (bidirectional).
pub async fn wire_hub_to_meta(
    hub_name: &str,
    hub: &Arc<Mutex<Hub>>,
    meta_hub: &Arc<Mutex<MetaHub>>,
) -> Arc<Connection> {
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let hub_conn = Arc::new(Connection::new(hub_transport));
    let meta_conn = Arc::new(Connection::new(meta_transport));
    let hub_rx = hub_conn.start();
    let meta_rx = meta_conn.start();

    {
        let mut mh = meta_hub.lock().await;
        mh.registry
            .register(hub_name, meta_conn.clone(), vec![])
            .unwrap();
    }

    let mh = meta_hub.clone();
    let meta_name = hub_name.to_string();
    tokio::spawn(async move {
        loopal_meta_hub::io_loop::meta_hub_io_loop(mh, meta_conn, meta_rx, meta_name).await;
    });

    // Use shared reverse handler from uplink module (no code duplication)
    let reverse_hub = hub.clone();
    let reverse_conn = hub_conn.clone();
    let reverse_name = hub_name.to_string();
    tokio::spawn(async move {
        loopal_agent_hub::uplink::handle_reverse_requests(
            reverse_hub,
            reverse_conn,
            hub_rx,
            reverse_name,
        )
        .await;
    });

    hub_conn
}

/// Register a mock agent with auto-responder. Returns (conn, forwarded_rx).
pub async fn register_mock_agent(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    parent: Option<&str>,
) -> (Arc<Connection>, mpsc::Receiver<Incoming>) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let server_conn = Arc::new(Connection::new(server_transport));
    let client_conn = Arc::new(Connection::new(client_transport));
    let server_rx = server_conn.start();
    let client_rx = client_conn.start();

    register_agent_connection(
        hub.clone(),
        name,
        server_conn,
        server_rx,
        parent,
        None,
        None,
    )
    .await;

    let cc = client_conn.clone();
    let mut listen_rx = client_rx;
    let (forward_tx, forward_rx) = mpsc::channel::<Incoming>(64);
    tokio::spawn(async move {
        while let Some(msg) = listen_rx.recv().await {
            if let Incoming::Request { id, .. } = &msg {
                let _ = cc.respond(*id, json!({"ok": true})).await;
            }
            let _ = forward_tx.send(msg).await;
        }
    });

    (client_conn, forward_rx)
}
