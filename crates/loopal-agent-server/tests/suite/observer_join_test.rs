//! Tests for observer client joining an active session and receiving events.
//! Validates the core of TUI auto-attach: agent/join -> observer_loop -> events flow.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_protocol::{AgentEvent, AgentEventPayload, Envelope, MessageSource};
use loopal_test_support::TestFixture;
use loopal_test_support::chunks;
use loopal_test_support::mock_provider::MultiCallProvider;

const T: Duration = Duration::from_secs(10);

pub(crate) fn make_duplex_pair() -> (Arc<dyn Transport>, Arc<dyn Transport>) {
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
    (server_t, client_t)
}

/// Drain notifications until AwaitingInput or Finished.
async fn drain_until_terminal(rx: &mut tokio::sync::mpsc::Receiver<Incoming>) {
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(ev) = serde_json::from_value::<AgentEvent>(params) {
                        if matches!(
                            ev.payload,
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        ) {
                            return;
                        }
                    }
                }
            }
            _ => return,
        }
    }
}

/// Collect agent/event notifications until terminal, return payloads.
async fn collect_events(rx: &mut tokio::sync::mpsc::Receiver<Incoming>) -> Vec<AgentEventPayload> {
    let mut events = Vec::new();
    let deadline = tokio::time::Instant::now() + T;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(Incoming::Notification { method, params })) => {
                if method == methods::AGENT_EVENT.name {
                    if let Ok(ev) = serde_json::from_value::<AgentEvent>(params) {
                        let terminal = matches!(
                            ev.payload,
                            AgentEventPayload::Finished | AgentEventPayload::AwaitingInput
                        );
                        events.push(ev.payload);
                        if terminal {
                            break;
                        }
                    }
                }
            }
            _ => break,
        }
    }
    events
}

/// Helper: initialize a client connection.
pub(crate) async fn init_client(conn: &Connection) {
    tokio::time::timeout(
        T,
        conn.send_request("initialize", serde_json::json!({"protocol_version": 1})),
    )
    .await
    .unwrap()
    .unwrap();
}

/// Observer joins active session and receives events from subsequent turns.
#[tokio::test]
#[ignore = "agent/join removed: Hub now handles multi-client observation"]
async fn observer_joins_session_receives_events() {
    let hub = Arc::new(loopal_agent_server::session_hub::SessionHub::new());
    let fixture = TestFixture::new();
    let provider = Arc::new(MultiCallProvider::new(vec![chunks::text_turn(
        "reply from agent",
    )])) as Arc<dyn loopal_provider_api::Provider>;
    hub.set_test_provider(provider).await;

    let (primary_server_t, primary_client_t) = make_duplex_pair();
    let (observer_server_t, observer_client_t) = make_duplex_pair();

    let h1 = hub.clone();
    tokio::spawn(async move {
        let _ = loopal_agent_server::run_test_connection(primary_server_t, h1).await;
    });
    let h2 = hub.clone();
    tokio::spawn(async move {
        let _ = loopal_agent_server::run_test_connection(observer_server_t, h2).await;
    });

    // Primary: init + start interactive (no prompt -> AwaitingInput)
    let primary = Arc::new(Connection::new(primary_client_t));
    let mut primary_rx = primary.start();
    init_client(&primary).await;
    let start_resp = tokio::time::timeout(
        T,
        primary.send_request(
            methods::AGENT_START.name,
            serde_json::json!({"cwd": fixture.path().to_string_lossy().as_ref()}),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(start_resp["session_id"].is_string());
    drain_until_terminal(&mut primary_rx).await;

    // Observer: init + join
    let observer = Arc::new(Connection::new(observer_client_t));
    let mut observer_rx = observer.start();
    init_client(&observer).await;
    let join_resp = tokio::time::timeout(
        T,
        observer.send_request(methods::AGENT_JOIN.name, serde_json::json!({})),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(join_resp["ok"], true, "agent/join should succeed");

    // Primary sends message -> triggers LLM -> events broadcast to observer
    let envelope = Envelope::new(MessageSource::Human, "main", "test message");
    tokio::time::timeout(
        T,
        primary.send_request(
            methods::AGENT_MESSAGE.name,
            serde_json::to_value(&envelope).unwrap(),
        ),
    )
    .await
    .unwrap()
    .unwrap();

    let events = collect_events(&mut observer_rx).await;
    assert!(!events.is_empty(), "observer should receive events");

    let has_stream = events
        .iter()
        .any(|e| matches!(e, AgentEventPayload::Stream { .. }));
    let has_terminal = events.iter().any(|e| {
        matches!(
            e,
            AgentEventPayload::AwaitingInput | AgentEventPayload::Finished
        )
    });
    assert!(has_stream, "observer should receive stream events");
    assert!(has_terminal, "observer should receive terminal event");
}
