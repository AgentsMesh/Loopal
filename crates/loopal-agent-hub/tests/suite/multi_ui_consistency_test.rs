//! Multi-UI consistency tests: user-input broadcast + permission/question
//! resolution propagation. Both flows are required for `--attach-hub` to
//! deliver the same conversation state to every connected UI.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use loopal_agent_hub::{Hub, agent_io, hub_server, start_event_loop};
use loopal_ipc::Connection;
use loopal_ipc::TcpTransport;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, Envelope, MessageSource, QualifiedAddress, UserContent,
};

async fn make_hub() -> (Arc<Mutex<Hub>>, u16, String) {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx.clone())));
    let (listener, port, token) = hub_server::start_hub_listener(hub.clone()).await.unwrap();
    let hub_accept = hub.clone();
    let token_for_loop = token.clone();
    tokio::spawn(async move {
        hub_server::accept_loop(listener, hub_accept, token_for_loop).await;
    });
    let _event_loop = start_event_loop(hub.clone(), raw_rx);
    (hub, port, token)
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
    conn.send_request(
        methods::HUB_REGISTER.name,
        json!({"name": name, "token": token, "role": "ui_client"}),
    )
    .await
    .unwrap();
    (conn, rx)
}

/// Register an in-process duplex agent. The returned `agent_side` is
/// the connection the *agent* uses to talk back; spawn a task on it
/// that auto-responds to `agent/message` so `routing::route_to_agent`
/// resolves promptly.
async fn register_auto_responding_agent(hub: &Arc<Mutex<Hub>>, name: &str) {
    let (t1, t2) = loopal_ipc::duplex_pair();
    let hub_side = Arc::new(Connection::new(t1));
    let _hub_rx = hub_side.start();
    let agent_side = Arc::new(Connection::new(t2));
    let mut agent_rx = agent_side.start();
    hub.lock()
        .await
        .registry
        .register_connection(name, hub_side)
        .unwrap();
    tokio::spawn(async move {
        while let Some(msg) = agent_rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = agent_side.respond(id, json!({"ok": true})).await;
            }
        }
    });
}

async fn next_event(rx: &mut mpsc::Receiver<Incoming>) -> Option<AgentEvent> {
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

async fn next_event_matching<F>(rx: &mut mpsc::Receiver<Incoming>, mut pred: F) -> AgentEvent
where
    F: FnMut(&AgentEventPayload) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let ev = tokio::time::timeout_at(deadline, next_event(rx))
            .await
            .expect("timeout waiting for matching event")
            .expect("stream closed");
        if pred(&ev.payload) {
            return ev;
        }
    }
}

#[tokio::test]
async fn user_input_from_one_ui_lands_in_both_ui_streams() {
    let (hub, port, token) = make_hub().await;
    register_auto_responding_agent(&hub, "main").await;

    let (ui_a, mut rx_a) = connect_ui(port, &token, "tui-A").await;
    let (_ui_b, mut rx_b) = connect_ui(port, &token, "tui-B").await;
    tokio::time::sleep(Duration::from_millis(80)).await;

    // UI A sends an envelope — same path `SessionController::route_message` takes.
    let envelope = Envelope::new(
        MessageSource::Human,
        QualifiedAddress::local("main"),
        UserContent::from("hello from A"),
    );
    ui_a.send_request(
        methods::HUB_ROUTE.name,
        serde_json::to_value(envelope).unwrap(),
    )
    .await
    .unwrap();

    let on_a = next_event_matching(&mut rx_a, |p| {
        matches!(p, AgentEventPayload::UserMessageQueued { .. })
    })
    .await;
    let on_b = next_event_matching(&mut rx_b, |p| {
        matches!(p, AgentEventPayload::UserMessageQueued { .. })
    })
    .await;

    for ev in [on_a, on_b] {
        let AgentEventPayload::UserMessageQueued { content, .. } = ev.payload else {
            panic!("expected UserMessageQueued");
        };
        assert_eq!(content, "hello from A");
    }
}

/// Spawn a task that responds to `agent/permission` relay requests
/// Send a `hub/permission_response` from `conn` as soon as a
/// `ToolPermissionRequest` event arrives on `rx` (parsed from
/// `agent/event` notifications).
fn approve_first_permission_via_events(
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Incoming>,
    allow: bool,
) {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let Incoming::Notification { method, params } = msg else {
                continue;
            };
            if method != methods::AGENT_EVENT.name {
                continue;
            }
            let Ok(event) = serde_json::from_value::<AgentEvent>(params) else {
                continue;
            };
            let agent = event
                .agent_name
                .as_ref()
                .map(|q| q.agent.clone())
                .unwrap_or_else(|| "main".to_string());
            if let AgentEventPayload::ToolPermissionRequest { id, .. } = event.payload {
                let _ = conn
                    .send_request(
                        methods::HUB_PERMISSION_RESPONSE.name,
                        json!({"agent_name": agent, "tool_call_id": id, "allow": allow}),
                    )
                    .await;
                return;
            }
        }
    });
}

#[tokio::test]
async fn permission_resolved_event_reaches_non_winning_ui() {
    let (hub, port, token) = make_hub().await;

    let (t1, t2) = loopal_ipc::duplex_pair();
    let hub_side = Arc::new(Connection::new(t1));
    let hub_rx = hub_side.start();
    let agent_side = Arc::new(Connection::new(t2));
    let _agent_rx = agent_side.start();
    hub.lock()
        .await
        .registry
        .register_connection("main", hub_side.clone())
        .unwrap();
    // Hub-side IO loop: dispatch agent/permission via pending_relay
    // (writes pending + emits ToolPermissionRequest event).
    agent_io::spawn_io_loop(hub.clone(), "main", hub_side, hub_rx);

    let (ui_a, rx_a) = connect_ui(port, &token, "tui-A").await;
    let (_ui_b, mut rx_b) = connect_ui(port, &token, "tui-B").await;
    tokio::time::sleep(Duration::from_millis(80)).await;

    // UI A approves whatever permission event arrives first.
    approve_first_permission_via_events(ui_a.clone(), rx_a, true);

    let perm = json!({
        "tool_call_id": "perm-1",
        "tool_name": "Bash",
        "tool_input": {"command": "ls"},
    });
    let resp = agent_side
        .send_request(methods::AGENT_PERMISSION.name, perm)
        .await
        .expect("agent gets permission response");
    assert_eq!(resp.get("allow").and_then(|v| v.as_bool()), Some(true));

    // UI B (the loser) must observe ToolPermissionResolved with id "perm-1".
    let resolved = next_event_matching(&mut rx_b, |p| {
        matches!(p, AgentEventPayload::ToolPermissionResolved { .. })
    })
    .await;
    let AgentEventPayload::ToolPermissionResolved { id } = resolved.payload else {
        panic!();
    };
    assert_eq!(id, "perm-1");
}
