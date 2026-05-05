//! Tests for HubFrontend multi-client behavior: broadcast, permission routing,
//! primary promotion, and interrupt handling.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEventPayload, InterruptSignal};
use loopal_runtime::frontend::traits::AgentFrontend;

use loopal_agent_server::session_hub::SharedSession;
use loopal_runtime::agent_input::AgentInput;

/// Create a bidirectional Connection pair (like a network socket pair).
/// Returns (server_conn, client_conn, client_rx).
fn conn_pair() -> (
    Arc<Connection>,
    Arc<Connection>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let server_t: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    let client_t: Arc<dyn Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let server_conn = Arc::new(Connection::new(server_t));
    let _server_rx = server_conn.start(); // Must start reader loop
    let client_conn = Arc::new(Connection::new(client_t));
    let client_rx = client_conn.start();
    (server_conn, client_conn, client_rx)
}

fn make_session() -> (
    Arc<SharedSession>,
    tokio::sync::mpsc::Receiver<AgentInput>,
    tokio::sync::watch::Receiver<u64>,
) {
    let (input_tx, input_rx) = tokio::sync::mpsc::channel(16);
    let interrupt = InterruptSignal::new();
    let (watch_tx, watch_rx) = tokio::sync::watch::channel(0u64);
    let session = Arc::new(SharedSession {
        session_id: "test-session".into(),
        clients: Mutex::new(Vec::new()),
        input_tx,
        interrupt,
        interrupt_tx: Arc::new(watch_tx),
        agent_shared: Mutex::new(None),
    });
    (session, input_rx, watch_rx)
}

const T: Duration = Duration::from_secs(5);

/// emit() broadcasts agent/event to ALL connected clients.
#[tokio::test]
async fn hub_emit_broadcasts_to_all_clients() {
    let (session, input_rx, watch_rx) = make_session();

    // Two clients
    let (srv_a, _cli_a, mut rx_a) = conn_pair();
    let (srv_b, _cli_b, mut rx_b) = conn_pair();
    session.add_client("a".into(), srv_a).await;
    session.add_client("b".into(), srv_b).await;

    let frontend =
        loopal_agent_server::hub_frontend::HubFrontend::new(session, input_rx, None, watch_rx);
    frontend
        .emit(AgentEventPayload::Stream {
            text: "hello".into(),
        })
        .await
        .unwrap();

    // Both clients should receive the notification
    for rx in [&mut rx_a, &mut rx_b] {
        let msg = tokio::time::timeout(T, rx.recv()).await.unwrap().unwrap();
        match msg {
            Incoming::Notification { method, params } => {
                assert_eq!(method, methods::AGENT_EVENT.name);
                // AgentEvent serializes as { "agent_name": null, "payload": { "Stream": { "text": ... } } }
                // or { "payload": { "type": "stream", "text": ... } } depending on serde config.
                // Just verify the method is correct and params are non-null.
                let event: loopal_protocol::AgentEvent =
                    serde_json::from_value(params).expect("should deserialize as AgentEvent");
                match event.payload {
                    AgentEventPayload::Stream { text } => assert_eq!(text, "hello"),
                    other => panic!("expected Stream, got {other:?}"),
                }
            }
            _ => panic!("expected notification"),
        }
    }
}

/// request_permission() sends to primary client only.
#[tokio::test]
async fn hub_permission_routes_to_primary() {
    let (session, input_rx, watch_rx) = make_session();

    let (srv_a, cli_a, mut rx_a) = conn_pair();
    let (srv_b, _cli_b, mut rx_b) = conn_pair();
    session.add_client("primary".into(), srv_a).await;
    session.add_client("observer".into(), srv_b).await;

    let frontend = Arc::new(loopal_agent_server::hub_frontend::HubFrontend::new(
        session, input_rx, None, watch_rx,
    ));

    let f2 = frontend.clone();
    let perm_task = tokio::spawn(async move {
        f2.request_permission("tc-1", "Bash", &serde_json::json!({"cmd": "ls"}))
            .await
    });

    // Primary should receive permission request
    let msg = tokio::time::timeout(T, rx_a.recv()).await.unwrap().unwrap();
    match msg {
        Incoming::Request { id, method, .. } => {
            assert_eq!(method, methods::AGENT_PERMISSION.name);
            cli_a
                .respond(id, serde_json::json!({"allow": true}))
                .await
                .unwrap();
        }
        _ => panic!("expected request on primary"),
    }

    let decision = tokio::time::timeout(T, perm_task).await.unwrap().unwrap();
    assert!(matches!(
        decision,
        loopal_tool_api::PermissionDecision::Allow
    ));

    // Observer should NOT have received anything
    let obs = tokio::time::timeout(Duration::from_millis(100), rx_b.recv()).await;
    assert!(
        obs.is_err(),
        "observer should not receive permission request"
    );
}

/// Primary client disconnects → next client promoted.
#[tokio::test]
async fn hub_primary_promotion_on_disconnect() {
    let (session, _input_rx, _watch_rx) = make_session();

    let (conn_a, _, _) = conn_pair();
    let (conn_b, _, _) = conn_pair();
    session.add_client("a".into(), conn_a).await;
    session.add_client("b".into(), conn_b).await;

    assert!(session.primary_connection().await.is_some());
    assert_eq!(session.all_connections().await.len(), 2);

    session.remove_client("a").await;
    assert!(session.primary_connection().await.is_some(), "b promoted");
    assert_eq!(session.all_connections().await.len(), 1);

    session.remove_client("b").await;
    assert!(session.primary_connection().await.is_none());
}

/// recv_input() returns None when interrupt watch fires.
#[tokio::test]
async fn hub_interrupt_wakes_recv_input() {
    let (session, input_rx, watch_rx) = make_session();
    let interrupt_tx = session.interrupt_tx.clone();

    let frontend =
        loopal_agent_server::hub_frontend::HubFrontend::new(session, input_rx, None, watch_rx);

    let recv_task = tokio::spawn(async move { frontend.recv_input().await });

    tokio::time::sleep(Duration::from_millis(50)).await;
    interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));

    let result = tokio::time::timeout(T, recv_task).await.unwrap().unwrap();
    assert!(result.is_none(), "should return None on interrupt");
}
