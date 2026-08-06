use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use loopal_agent_hub::{Hub, HubClient, hub_server, start_event_loop};
use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;

async fn make_hub_with_tcp() -> (Arc<Mutex<Hub>>, u16, String) {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx)));
    let (listener, port, token) = hub_server::start_hub_listener(hub.clone()).await.unwrap();
    let hub_accept = hub.clone();
    let token_for_loop = token.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, token_for_loop).await;
    });
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    (hub, port, token)
}

async fn connect_and_register(port: u16, token: &str, name: &str) -> Arc<Connection<Listening>> {
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, _rx) = Connection::new(transport).into_listening();
    let resp = conn
        .send_request(
            methods::HUB_REGISTER.name,
            json!({"name": name, "token": token, "role": "ui_client"}),
        )
        .await
        .unwrap();
    assert!(resp.get("ok").is_some(), "register failed: {resp:?}");
    conn
}

async fn wait_until_unregistered(hub: &Arc<Mutex<Hub>>, name: &str, max_wait: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + max_wait;
    while tokio::time::Instant::now() < deadline {
        if !hub.lock().await.ui.has_client_name(name) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

#[tokio::test]
async fn tcp_ui_disconnect_unregisters_client() {
    let (hub, port, token) = make_hub_with_tcp().await;

    let conn = connect_and_register(port, &token, "tui-cleanup").await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        hub.lock().await.ui.has_client_name("tui-cleanup"),
        "client should be registered while connection is alive"
    );

    conn.close().await;

    assert!(
        wait_until_unregistered(&hub, "tui-cleanup", Duration::from_secs(2)).await,
        "client should be unregistered after Connection close"
    );
}

#[tokio::test]
async fn dropping_owned_hub_client_revokes_capable_tcp_lease() {
    let (hub, port, token) = make_hub_with_tcp().await;
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, _rx) = Connection::new(transport).into_listening();
    conn.send_request(
        methods::HUB_REGISTER.name,
        json!({
            "name": "owned-tui",
            "token": token,
            "role": "ui_client",
            "capabilities": loopal_protocol::UiCapabilities::ALL,
        }),
    )
    .await
    .unwrap();
    assert!(hub.lock().await.ui.has_client_name("owned-tui"));

    drop(HubClient::new_with_transport_lease(conn));

    assert!(wait_until_unregistered(&hub, "owned-tui", Duration::from_secs(2)).await);
    assert!(
        !hub.lock()
            .await
            .ui
            .has_capability(loopal_protocol::UiCapability::PlanApproval)
    );
}

#[tokio::test]
async fn tcp_ui_reconnect_with_same_name_after_disconnect() {
    let (hub, port, token) = make_hub_with_tcp().await;

    let conn1 = connect_and_register(port, &token, "tui-reconn").await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    conn1.close().await;
    assert!(wait_until_unregistered(&hub, "tui-reconn", Duration::from_secs(2)).await);

    let _conn2 = connect_and_register(port, &token, "tui-reconn").await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        hub.lock().await.ui.has_client_name("tui-reconn"),
        "second connection with same name should re-register cleanly"
    );
}

#[tokio::test]
async fn multiple_ui_clients_independent_cleanup() {
    let (hub, port, token) = make_hub_with_tcp().await;

    let c1 = connect_and_register(port, &token, "tui-a").await;
    let _c2 = connect_and_register(port, &token, "tui-b").await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    {
        let h = hub.lock().await;
        assert!(h.ui.has_client_name("tui-a"));
        assert!(h.ui.has_client_name("tui-b"));
    }

    c1.close().await;
    assert!(wait_until_unregistered(&hub, "tui-a", Duration::from_secs(2)).await);
    assert!(
        hub.lock().await.ui.has_client_name("tui-b"),
        "tui-b must remain registered when tui-a disconnects"
    );
}

#[tokio::test]
async fn register_ack_observes_installed_lease_and_capability() {
    let (hub, port, token) = make_hub_with_tcp().await;
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, _rx) = Connection::new(transport).into_listening();

    let response = conn
        .send_request(
            methods::HUB_REGISTER.name,
            json!({
                "name": "ready-ui",
                "token": token,
                "role": "ui_client",
                "capabilities": {"plan_approval": true},
            }),
        )
        .await
        .unwrap();
    let lease = response["lease_id"].as_str().unwrap();
    let h = hub.lock().await;
    assert!(h.ui.is_ui_client(lease));
    assert!(h.ui.client_has_capability(lease, loopal_protocol::UiCapability::PlanApproval));
}
