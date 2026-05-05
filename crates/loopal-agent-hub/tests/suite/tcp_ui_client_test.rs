//! Integration test: TCP UI client registration + agent/event broadcast.
//!
//! Verifies that `hub/register` with `role: "ui_client"` routes the
//! connection through `tcp_ui_io` (not `agent_io`), and that subsequent
//! `AgentEvent`s broadcast on the hub are forwarded to the TCP client
//! as `agent/event` notifications.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use loopal_agent_hub::{Hub, hub_server, start_event_loop};
use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload};

async fn make_hub_with_tcp() -> (Arc<Mutex<Hub>>, mpsc::Sender<AgentEvent>, u16, String) {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx.clone())));
    let (listener, port, token) = hub_server::start_hub_listener(hub.clone()).await.unwrap();
    let hub_accept = hub.clone();
    let token_for_loop = token.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, token_for_loop).await;
    });
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    (hub, raw_tx, port, token)
}

async fn connect_ui_client(
    port: u16,
    token: &str,
    name: &str,
) -> (Arc<Connection>, mpsc::Receiver<Incoming>) {
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("tcp connect");
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let rx = conn.start();
    let resp = conn
        .send_request(
            methods::HUB_REGISTER.name,
            json!({"name": name, "token": token, "role": "ui_client"}),
        )
        .await
        .expect("register sends");
    assert!(
        resp.get("ok").is_some(),
        "expected ok response, got: {resp:?}"
    );
    (conn, rx)
}

#[tokio::test]
async fn ui_client_role_receives_broadcast_events() {
    let (_hub, raw_tx, port, token) = make_hub_with_tcp().await;
    let (_conn, mut rx) = connect_ui_client(port, &token, "tui-1").await;

    // Allow forward task to subscribe before we emit.
    tokio::time::sleep(Duration::from_millis(50)).await;

    raw_tx
        .send(AgentEvent::root(AgentEventPayload::Running))
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(2), recv_agent_event(&mut rx))
        .await
        .expect("timeout waiting for agent/event")
        .expect("event missing");
    assert!(matches!(received.payload, AgentEventPayload::Running));
}

#[tokio::test]
async fn two_ui_clients_both_receive_events() {
    let (_hub, raw_tx, port, token) = make_hub_with_tcp().await;
    let (_c1, mut r1) = connect_ui_client(port, &token, "tui-1").await;
    let (_c2, mut r2) = connect_ui_client(port, &token, "tui-2").await;

    tokio::time::sleep(Duration::from_millis(80)).await;

    raw_tx
        .send(AgentEvent::root(AgentEventPayload::Stream {
            text: "hello".into(),
        }))
        .await
        .unwrap();

    for rx in [&mut r1, &mut r2] {
        let ev = tokio::time::timeout(Duration::from_secs(2), recv_agent_event(rx))
            .await
            .expect("timeout")
            .expect("event missing");
        assert!(matches!(ev.payload, AgentEventPayload::Stream { .. }));
    }
}

#[tokio::test]
async fn unknown_role_is_rejected() {
    let (_hub, _raw_tx, port, token) = make_hub_with_tcp().await;
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let _rx = conn.start();
    let resp = conn
        .send_request(
            methods::HUB_REGISTER.name,
            json!({"name": "weird", "token": token, "role": "snitch"}),
        )
        .await
        .unwrap();
    assert!(
        resp.get("message").is_some(),
        "expected error, got: {resp:?}"
    );
}

async fn recv_agent_event(rx: &mut mpsc::Receiver<Incoming>) -> Option<AgentEvent> {
    while let Some(msg) = rx.recv().await {
        if let Incoming::Notification { method, params } = msg
            && method == methods::AGENT_EVENT.name
            && let Ok(ev) = serde_json::from_value::<AgentEvent>(params)
        {
            return Some(ev);
        }
    }
    None
}
