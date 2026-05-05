//! Mid-session attach test.
//!
//! UI B joins after UI A and the agent have already exchanged events.
//! Both UIs must observe the same subsequent events with identical Hub-
//! stamped `rev` values. Late-joining UI also pulls `view/snapshot` (see
//! `view_snapshot_seed_test.rs`) to seed history before this point.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use loopal_agent_hub::{Hub, hub_server, start_event_loop};
use loopal_ipc::Connection;
use loopal_ipc::TcpTransport;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload, QualifiedAddress};

async fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Sender<AgentEvent>, u16, String) {
    let (raw_tx, raw_rx) = mpsc::channel(64);
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

async fn register_test_agent(hub: &Arc<Mutex<Hub>>, name: &str) {
    let (_t1, t2) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(t2));
    let _rx = conn.start();
    hub.lock()
        .await
        .registry
        .register_connection(name, conn)
        .expect("register agent");
}

async fn connect_ui(
    port: u16,
    token: &str,
    name: &str,
) -> (Arc<Connection>, mpsc::Receiver<Incoming>) {
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let conn = Arc::new(Connection::new(transport));
    let rx = conn.start();
    let resp = conn
        .send_request(
            methods::HUB_REGISTER.name,
            json!({"name": name, "token": token, "role": "ui_client"}),
        )
        .await
        .unwrap();
    assert!(resp.get("ok").is_some(), "register: {resp:?}");
    (conn, rx)
}

fn named(agent: &str, payload: AgentEventPayload) -> AgentEvent {
    AgentEvent::named(QualifiedAddress::local(agent), payload)
}

async fn next_event_with_text(
    rx: &mut mpsc::Receiver<Incoming>,
    target: &str,
    max: Duration,
) -> Option<AgentEvent> {
    let deadline = tokio::time::Instant::now() + max;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params }))
                if method == methods::AGENT_EVENT.name =>
            {
                let Ok(ev) = serde_json::from_value::<AgentEvent>(params) else {
                    continue;
                };
                if let AgentEventPayload::Stream { ref text } = ev.payload
                    && text == target
                {
                    return Some(ev);
                }
            }
            Ok(Some(_)) => continue,
            _ => continue,
        }
    }
    None
}

#[tokio::test]
async fn mid_attach_ui_receives_subsequent_events() {
    let (hub, raw_tx, port, token) = make_hub().await;
    register_test_agent(&hub, "worker").await;
    let (_a, mut rx_a) = connect_ui(port, &token, "tui-A").await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    raw_tx
        .send(named(
            "worker",
            AgentEventPayload::Stream {
                text: "first".into(),
            },
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // UI B attaches after UI A already saw "first".
    let (_b, mut rx_b) = connect_ui(port, &token, "tui-B").await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    raw_tx
        .send(named(
            "worker",
            AgentEventPayload::Stream {
                text: "second".into(),
            },
        ))
        .await
        .unwrap();

    let on_a = next_event_with_text(&mut rx_a, "second", Duration::from_secs(2))
        .await
        .expect("UI A should see second");
    let on_b = next_event_with_text(&mut rx_b, "second", Duration::from_secs(2))
        .await
        .expect("UI B should see second");

    assert_eq!(on_a.rev, on_b.rev, "rev must be identical across UIs");
    assert!(on_a.rev.is_some(), "Hub must stamp rev on broadcast events");
}

/// Subsequent events to two attached UIs must arrive in the same order
/// with monotonically increasing rev — the foundation of cross-UI state
/// convergence.
#[tokio::test]
async fn rev_strictly_monotonic_across_subsequent_events() {
    let (hub, raw_tx, port, token) = make_hub().await;
    register_test_agent(&hub, "worker").await;
    let (_a, mut rx_a) = connect_ui(port, &token, "tui-A").await;
    let (_b, mut rx_b) = connect_ui(port, &token, "tui-B").await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    for tag in ["e1", "e2", "e3"] {
        raw_tx
            .send(named(
                "worker",
                AgentEventPayload::Stream { text: tag.into() },
            ))
            .await
            .unwrap();
    }

    let mut revs_a = Vec::new();
    let mut revs_b = Vec::new();
    for tag in ["e1", "e2", "e3"] {
        let a = next_event_with_text(&mut rx_a, tag, Duration::from_secs(2))
            .await
            .unwrap_or_else(|| panic!("UI A missing {tag}"));
        let b = next_event_with_text(&mut rx_b, tag, Duration::from_secs(2))
            .await
            .unwrap_or_else(|| panic!("UI B missing {tag}"));
        assert_eq!(a.rev, b.rev, "rev mismatch on {tag}");
        revs_a.push(a.rev.unwrap());
        revs_b.push(b.rev.unwrap());
    }

    assert!(
        revs_a.windows(2).all(|w| w[0] < w[1]),
        "rev must be strictly increasing: {revs_a:?}"
    );
    assert_eq!(revs_a, revs_b);
}
