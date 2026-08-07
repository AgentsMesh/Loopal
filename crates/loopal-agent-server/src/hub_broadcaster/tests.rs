use std::sync::Arc;
use std::task::Poll;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::InterruptSignal;

use super::*;

fn session() -> Arc<SharedSession> {
    let (input_tx, _input_rx) = tokio::sync::mpsc::channel(1);
    let (interrupt_tx, _interrupt_rx) = tokio::sync::watch::channel(0);
    Arc::new(SharedSession::placeholder(
        input_tx,
        InterruptSignal::new(),
        Arc::new(interrupt_tx),
    ))
}

fn stream(text: &str) -> AgentEventPayload {
    AgentEventPayload::Stream {
        text: text.to_string(),
    }
}

fn stream_text(message: Incoming) -> String {
    let Incoming::Notification { method, params } = message else {
        panic!("expected agent event notification");
    };
    assert_eq!(method, loopal_ipc::protocol::methods::AGENT_EVENT.name);
    let event: AgentEvent = serde_json::from_value(params).expect("valid AgentEvent payload");
    let AgentEventPayload::Stream { text } = event.payload else {
        panic!("expected stream event");
    };
    text
}

#[tokio::test]
async fn critical_broadcast_propagates_delivery_failure() {
    let session_ref = Arc::new(tokio::sync::RwLock::new(session()));
    let broadcaster = HubBroadcaster::new(session_ref, None);

    let error = broadcaster.broadcast(stream("critical")).await.unwrap_err();
    assert!(matches!(error, LoopalError::Ipc(message) if message.contains("no connected clients")));
}

#[tokio::test]
async fn try_broadcast_queues_during_session_lock_contention_and_stays_ordered() {
    let session = session();
    let (peer, server) = loopal_ipc::duplex_pair();
    let (server, _server_incoming) = Connection::new(server).into_listening();
    let (_peer, mut incoming) = Connection::new(peer).into_listening();
    session.add_client("primary".into(), server).await;

    let session_ref = Arc::new(tokio::sync::RwLock::new(session));
    let broadcaster = HubBroadcaster::new(session_ref.clone(), None);
    let clone = broadcaster.clone();
    let session_write = session_ref.write().await;

    assert!(
        broadcaster.try_broadcast(stream("first")),
        "lock contention must not reject an event that can be queued"
    );
    let mut second = std::pin::pin!(clone.broadcast(stream("second")));
    assert!(matches!(futures::poll!(&mut second), Poll::Pending));
    assert!(matches!(
        incoming.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    drop(session_write);
    second.await.unwrap();

    let first = incoming.recv().await.expect("first event");
    let second = incoming.recv().await.expect("second event");
    assert_eq!(stream_text(first), "first");
    assert_eq!(stream_text(second), "second");
}
