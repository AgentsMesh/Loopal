//! E2E tests with real TCP connections (not DuplexTransport).
//! Verifies the entire cluster communication stack over TCP.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_agent_hub::{Hub, HubUplink};
use loopal_ipc::connection::{Connection, Incoming};
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
) -> Arc<Connection> {
    let stream = TcpStream::connect(meta_addr).await.expect("TCP connect");
    let transport: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let rx = conn.start();

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
) -> (Arc<Connection>, mpsc::Receiver<Incoming>) {
    let (client_t, server_t) = loopal_ipc::duplex_pair();
    let server = Arc::new(Connection::new(server_t));
    let client = Arc::new(Connection::new(client_t));
    let server_rx = server.start();
    let client_rx = client.start();

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

/// Boot a real TCP cluster and route a message across hubs.
#[tokio::test]
async fn tcp_cluster_cross_hub_route() {
    let (addr, token, _meta_hub) = boot_meta_hub().await;
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();

    let _conn_a = join_hub_tcp(&hub_a, &addr, &token, "hub-a").await;
    let _conn_b = join_hub_tcp(&hub_b, &addr, &token, "hub-b").await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register agent on Hub-B
    let (_agent_conn, _agent_rx) = register_mock(&hub_b, "target").await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Route from Hub-A to Hub-B's agent via uplink → MetaHub → Hub-B
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": {"hub": [], "agent": "sender"}},
        "target": {"hub": ["hub-b"], "agent": "target"},
        "content": {"text": "hello via TCP", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "sender".into(),
    )
    .await;

    assert!(
        result.is_ok(),
        "TCP cross-hub route should succeed: {result:?}"
    );
}

#[tokio::test]
async fn tcp_cluster_list_hubs() {
    let (addr, token, meta_hub) = boot_meta_hub().await;
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();

    let _conn_a = join_hub_tcp(&hub_a, &addr, &token, "hub-a").await;
    let _conn_b = join_hub_tcp(&hub_b, &addr, &token, "hub-b").await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::META_LIST_HUBS.name,
        json!({}),
        "hub-a".into(),
    )
    .await
    .unwrap();

    let hubs = result["hubs"].as_array().unwrap();
    assert_eq!(hubs.len(), 2);
    let names: Vec<&str> = hubs.iter().filter_map(|h| h["name"].as_str()).collect();
    assert!(names.contains(&"hub-a"));
    assert!(names.contains(&"hub-b"));
}
