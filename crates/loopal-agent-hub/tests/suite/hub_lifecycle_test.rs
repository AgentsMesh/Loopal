//! Hub lifecycle tests: shutdown, disconnect, token authentication.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::hub_server;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<AgentHub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(AgentHub::new(tx))), rx)
}

fn spawn_mock_agent(conn: Arc<Connection>, mut rx: mpsc::Receiver<Incoming>) {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = conn.respond(id, json!({"ok": true})).await;
            }
        }
    });
}

#[tokio::test]
async fn shutdown_agent_sends_shutdown_request() {
    let (hub, _) = make_hub();

    let (sender, sr) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock_agent(sender.clone(), sr);

    let (target_conn, target_rx) = hub_server::connect_local(hub.clone(), "victim");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let tc = target_conn.clone();
    tokio::spawn(async move {
        let mut rx = target_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = tc.respond(id, json!({"ok": true})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = sender
        .send_request(
            methods::HUB_SHUTDOWN_AGENT.name,
            json!({"target": "victim"}),
        )
        .await;
    assert!(result.is_ok());

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(method.unwrap().unwrap(), methods::AGENT_SHUTDOWN.name);
}

#[tokio::test]
async fn tcp_invalid_token_rejected() {
    let (hub, _) = make_hub();

    let (listener, port, _valid_token) = hub_server::start_hub_listener(hub.clone()).await.unwrap();
    let hub_bg = hub.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_bg, "correct-token".into()).await;
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn loopal_ipc::Transport> = Arc::new(loopal_ipc::TcpTransport::new(stream));
    let conn = Arc::new(loopal_ipc::Connection::new(transport));
    let _rx = conn.start();

    let result = conn
        .send_request(
            methods::HUB_REGISTER.name,
            json!({"name": "hacker", "token": "wrong-token"}),
        )
        .await;

    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(
        val.get("code").is_some() || val.get("message").is_some(),
        "should contain error: {val}"
    );

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(hub.lock().await.get_agent_connection("hacker").is_none());
}
