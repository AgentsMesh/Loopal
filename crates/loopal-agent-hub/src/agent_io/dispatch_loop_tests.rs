use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentCompletion, AgentEvent, AgentEventPayload, PROTOCOL_ERROR_REASON, TRANSPORT_ERROR_REASON,
};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

use super::dispatch_loop::{agent_io_loop, agent_io_loop_exact};
use crate::Hub;

fn hub() -> Arc<Mutex<Hub>> {
    let (events, _rx) = mpsc::channel::<AgentEvent>(8);
    Arc::new(Mutex::new(Hub::new(events)))
}

#[tokio::test]
async fn io_loop_rejects_unregistered_connection_lease() {
    let hub = hub();
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (connection, incoming) = Connection::new(transport).into_listening();
    let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));

    let completion = agent_io_loop(hub, dispatcher, connection, incoming, "missing".into()).await;

    assert_eq!(completion.reason, PROTOCOL_ERROR_REASON);
    assert!(completion.output().contains("active connection lease"));
}

async fn registered_loop(
    event_receiver_alive: bool,
) -> (
    Arc<Mutex<Hub>>,
    Arc<Connection<loopal_ipc::Listening>>,
    Arc<Connection<loopal_ipc::Listening>>,
    crate::types::AgentExecutionRef,
    Arc<loopal_ipc::Dispatcher>,
    mpsc::Sender<Incoming>,
    mpsc::Receiver<Incoming>,
) {
    let (events, event_rx) = mpsc::channel::<AgentEvent>(8);
    if event_receiver_alive {
        tokio::spawn(async move {
            let mut event_rx = event_rx;
            while event_rx.recv().await.is_some() {}
        });
    } else {
        drop(event_rx);
    }
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (peer_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (peer, _peer_incoming) = Connection::new(peer_transport).into_listening();
    let (connection, _connection_incoming) = Connection::new(hub_transport).into_listening();
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("worker", connection.clone(), None, None, None)
        .unwrap();
    let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));
    let (tx, rx) = mpsc::channel(4);
    (hub, connection, peer, execution, dispatcher, tx, rx)
}

#[tokio::test]
async fn exact_loop_returns_a_valid_completed_result() {
    let (hub, connection, _peer, execution, dispatcher, tx, rx) = registered_loop(true).await;
    tx.send(Incoming::Notification {
        method: methods::AGENT_COMPLETED.name.into(),
        params: serde_json::to_value(AgentCompletion::goal(Some("done".into()))).unwrap(),
    })
    .await
    .unwrap();

    let completion =
        agent_io_loop_exact(hub, dispatcher, connection, rx, "worker".into(), execution).await;

    assert_eq!(completion.output(), "done");
}

#[tokio::test]
async fn malformed_cancel_is_ignored_until_transport_closes() {
    let (hub, connection, _peer, execution, dispatcher, tx, rx) = registered_loop(true).await;
    tx.send(Incoming::Notification {
        method: methods::REQUEST_CANCEL.name.into(),
        params: json!({"id": "not-an-integer"}),
    })
    .await
    .unwrap();
    drop(tx);

    let completion =
        agent_io_loop_exact(hub, dispatcher, connection, rx, "worker".into(), execution).await;

    assert_eq!(completion.reason, TRANSPORT_ERROR_REASON);
    assert!(
        completion
            .output()
            .contains("closed before agent/completed")
    );
}

#[tokio::test]
async fn authoritative_event_delivery_failure_stops_the_loop() {
    let (hub, connection, _peer, execution, dispatcher, tx, rx) = registered_loop(false).await;
    tx.send(Incoming::Notification {
        method: methods::AGENT_EVENT.name.into(),
        params: serde_json::to_value(AgentEvent::root(AgentEventPayload::Running)).unwrap(),
    })
    .await
    .unwrap();

    let completion =
        agent_io_loop_exact(hub, dispatcher, connection, rx, "worker".into(), execution).await;

    assert_eq!(completion.reason, TRANSPORT_ERROR_REASON);
    assert!(completion.output().contains("event delivery failed"));
}
