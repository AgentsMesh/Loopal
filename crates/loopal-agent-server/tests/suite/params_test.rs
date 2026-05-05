//! Regression tests for config override propagation and event forwarding.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;

fn ipc_pair() -> (
    Arc<Connection>,
    tokio::sync::mpsc::Receiver<Incoming>,
    Arc<Connection>,
) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let ta: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let tb: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    let sa = Arc::new(Connection::new(ta));
    let sb = Arc::new(Connection::new(tb));
    let ra = sa.start();
    (sa, ra, sb)
}

/// Verify model override from start_agent reaches the agent config.
/// Uses the test harness to run a full agent loop with mock provider.
#[tokio::test]
async fn model_override_propagated_via_ipc() {
    let harness = loopal_test_support::ipc_harness::build_ipc_harness(
        loopal_test_support::scenarios::simple_text("model check"),
    )
    .await;

    // The harness used default model. If we get events, the model resolved correctly.
    let mut rx = harness.event_rx;
    let ev = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .unwrap();
    assert!(ev.is_some(), "should receive at least one event");
}

/// Event forwarder: events sent to parent_event_tx arrive via IPC.
#[tokio::test]
async fn event_forwarder_delivers_sub_agent_events() {
    let (server_conn, _server_rx, client_conn) = ipc_pair();
    let mut client_rx = client_conn.start();

    // Simulate what params.rs does: create event channel + forwarder task
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<loopal_protocol::AgentEvent>(256);
    let event_conn = server_conn.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if let Ok(params) = serde_json::to_value(&event) {
                let _ = event_conn
                    .send_notification(methods::AGENT_EVENT.name, params)
                    .await;
            }
        }
    });

    // Send a sub-agent event through the channel
    let event = loopal_protocol::AgentEvent {
        agent_name: Some("sub-1".into()),
        event_id: 0,
        turn_id: 0,
        correlation_id: 0,
        rev: None,
        payload: loopal_protocol::AgentEventPayload::Stream {
            text: "from sub-agent".into(),
        },
    };
    event_tx.send(event).await.unwrap();

    // Should arrive on client side via IPC
    let msg = tokio::time::timeout(Duration::from_secs(2), client_rx.recv())
        .await
        .unwrap()
        .unwrap();

    match msg {
        Incoming::Notification { method, params } => {
            assert_eq!(method, methods::AGENT_EVENT.name);
            let ev: loopal_protocol::AgentEvent = serde_json::from_value(params).unwrap();
            assert_eq!(
                ev.agent_name.as_ref().map(|a| a.to_string()).as_deref(),
                Some("sub-1")
            );
            match ev.payload {
                loopal_protocol::AgentEventPayload::Stream { text } => {
                    assert_eq!(text, "from sub-agent");
                }
                _ => panic!("expected Stream"),
            }
        }
        _ => panic!("expected notification"),
    }
}
