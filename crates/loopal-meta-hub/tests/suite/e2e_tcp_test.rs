//! E2E tests with real TCP connections (not DuplexTransport).
//! Verifies the entire cluster communication stack over TCP.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_agent_hub::{Hub, HubUplink};
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::tcp::TcpTransport;
use loopal_protocol::AgentEvent;
use serde_json::json;

use loopal_meta_hub::MetaHub;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Boot a real MetaHub TCP server. Returns (addr, token, meta_hub).
async fn boot_meta_hub() -> (String, String, Arc<Mutex<MetaHub>>) {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (listener, token) = loopal_meta_hub::server::start_meta_listener("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().unwrap().to_string();

    let mh = meta_hub.clone();
    let t = token.clone();
    tokio::spawn(async move {
        loopal_meta_hub::server::meta_accept_loop(listener, mh, t).await;
    });

    (addr, token, meta_hub)
}

/// Connect a Hub to MetaHub via real TCP.
async fn join_hub_tcp(
    hub: &Arc<Mutex<Hub>>,
    meta_addr: &str,
    token: &str,
    hub_name: &str,
) -> Arc<Connection<Listening>> {
    let stream = TcpStream::connect(meta_addr).await.expect("TCP connect");
    let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, rx) = Connection::new(transport).into_listening();

    // meta/register
    let resp = conn
        .send_request(
            methods::META_REGISTER.name,
            json!({"name": hub_name, "token": token, "capabilities": []}),
        )
        .await
        .expect("meta/register");
    assert_eq!(resp["ok"].as_bool(), Some(true));

    // Set uplink + start reverse handler
    let uplink = Arc::new(HubUplink::new(conn.clone(), hub_name.into()));
    hub.lock().await.uplink = Some(uplink);

    let reverse_hub = hub.clone();
    let reverse_conn = conn.clone();
    let reverse_name = hub_name.to_string();
    tokio::spawn(async move {
        loopal_agent_hub::uplink::handle_reverse_requests(
            reverse_hub,
            reverse_conn,
            rx,
            reverse_name,
        )
        .await;
    });

    conn
}

/// Register a mock agent with auto-responder.
async fn register_mock(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
) -> (Arc<Connection<Listening>>, mpsc::Receiver<Incoming>) {
    let (client_t, server_t) = loopal_ipc::duplex_pair();
    let (server, server_rx) = Connection::new(server_t).into_listening();
    let (client, client_rx) = Connection::new(client_t).into_listening();

    let _ = register_agent_connection(hub.clone(), name, server, server_rx, None, None, None)
        .await
        .unwrap();

    let cc = client.clone();
    let mut listen = client_rx;
    let (fwd_tx, fwd_rx) = mpsc::channel::<Incoming>(64);
    tokio::spawn(async move {
        while let Some(msg) = listen.recv().await {
            if let Incoming::Request { id, .. } = &msg {
                let _ = cc.respond(*id, json!({"ok": true})).await;
            }
            let _ = fwd_tx.send(msg).await;
        }
    });

    (client, fwd_rx)
}

include!("e2e_tcp_test/cluster_cases.rs");
