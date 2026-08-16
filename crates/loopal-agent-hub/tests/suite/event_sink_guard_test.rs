use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_agent_hub::{Hub, start_event_loop};
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn oversized_agent_event_never_reaches_broadcast_or_view_state() {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx)));
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (server, server_rx) = Connection::new(hub_transport).into_listening();
    register_agent_connection(hub.clone(), "worker", server, server_rx, None, None, None)
        .await
        .unwrap();
    let view = hub.lock().await.registry.agent_view("worker").unwrap();
    let mut events = hub.lock().await.ui.subscribe_events();
    let _event_loop = start_event_loop(hub, raw_rx);

    let event = AgentEvent::named(
        "worker",
        AgentEventPayload::Stream {
            text: "canary".repeat(loopal_output_guard::MAX_AGENT_EVENT_PAYLOAD_BYTES / 6 + 1),
        },
    );
    agent
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::to_value(event).unwrap(),
        )
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let received = events.recv().await.unwrap();
            let encoded = serde_json::to_string(&received).unwrap();
            assert!(!encoded.contains("canary"));
            if matches!(received.payload, AgentEventPayload::Error { .. }) {
                break;
            }
        }
    })
    .await
    .unwrap();
    let encoded_view = serde_json::to_string(view.lock().await.state()).unwrap();
    assert!(!encoded_view.contains("canary"));
}
