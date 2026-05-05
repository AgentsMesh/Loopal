//! End-to-end tests for `view/snapshot` — UI clients pull initial
//! ViewState; incremental updates flow through the `agent/event`
//! broadcast (covered separately).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{Hub, UiSession, start_event_loop};
use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, AgentStatus, QualifiedAddress};
use loopal_view_state::{ViewSnapshot, ViewSnapshotRequest};

fn make_hub() -> (
    Arc<Mutex<Hub>>,
    mpsc::Sender<AgentEvent>,
    mpsc::Receiver<AgentEvent>,
) {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx.clone())));
    (hub, raw_tx, raw_rx)
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

fn named_event(agent: &str, payload: AgentEventPayload) -> AgentEvent {
    AgentEvent::named(QualifiedAddress::local(agent), payload)
}

#[tokio::test]
async fn view_snapshot_returns_current_state() {
    let (hub, raw_tx, raw_rx) = make_hub();
    register_test_agent(&hub, "worker").await;
    let _handle = start_event_loop(hub.clone(), raw_rx);

    raw_tx
        .send(named_event("worker", AgentEventPayload::Running))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let ui = UiSession::connect(hub.clone(), "tui-1").await;
    let req = ViewSnapshotRequest {
        agent: "worker".into(),
    };
    let response = ui
        .client
        .connection()
        .send_request(
            methods::VIEW_SNAPSHOT.name,
            serde_json::to_value(req).unwrap(),
        )
        .await
        .expect("snapshot ok");
    let snapshot: ViewSnapshot = serde_json::from_value(response).expect("parse snapshot");

    assert_eq!(snapshot.rev, 1);
    assert_eq!(snapshot.state.agent.observable.status, AgentStatus::Running);
}

#[tokio::test]
async fn view_snapshot_unknown_agent_returns_error() {
    let (hub, _raw_tx, raw_rx) = make_hub();
    let _handle = start_event_loop(hub.clone(), raw_rx);

    let ui = UiSession::connect(hub.clone(), "tui-1").await;
    let req = ViewSnapshotRequest {
        agent: "ghost".into(),
    };
    let response = ui
        .client
        .connection()
        .send_request(
            methods::VIEW_SNAPSHOT.name,
            serde_json::to_value(req).unwrap(),
        )
        .await
        .expect("transport ok");
    let parse_attempt: Result<ViewSnapshot, _> = serde_json::from_value(response.clone());
    assert!(
        parse_attempt.is_err(),
        "expected error response, got: {response:?}"
    );
    assert!(
        response.get("message").is_some(),
        "expected JSON-RPC error.message, got: {response:?}"
    );
}
