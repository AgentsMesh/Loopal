//! Tests for the hub event router.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{AgentHub, start_event_loop};
use loopal_protocol::{AgentEvent, AgentEventPayload};

/// Normal events are forwarded to the frontend channel.
#[tokio::test]
async fn forwards_events_to_frontend() {
    let hub = Arc::new(Mutex::new(AgentHub::noop()));
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let (frontend_tx, mut frontend_rx) = mpsc::channel(16);

    let _handle = start_event_loop(hub, raw_rx, frontend_tx);

    let event = AgentEvent::root(AgentEventPayload::Stream {
        text: "hello".into(),
    });
    raw_tx.send(event).await.unwrap();

    let received = tokio::time::timeout(Duration::from_millis(100), frontend_rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    assert!(matches!(received.payload, AgentEventPayload::Stream { .. }));
}

/// Multiple events arrive in order.
#[tokio::test]
async fn preserves_event_order() {
    let hub = Arc::new(Mutex::new(AgentHub::noop()));
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let (frontend_tx, mut frontend_rx) = mpsc::channel(16);

    let _handle = start_event_loop(hub, raw_rx, frontend_tx);

    for i in 0..5 {
        let event = AgentEvent::root(AgentEventPayload::Stream {
            text: format!("msg-{i}"),
        });
        raw_tx.send(event).await.unwrap();
    }

    for i in 0..5 {
        let received = tokio::time::timeout(Duration::from_millis(100), frontend_rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");
        if let AgentEventPayload::Stream { text } = received.payload {
            assert_eq!(text, format!("msg-{i}"));
        } else {
            panic!("unexpected payload");
        }
    }
}

/// Loop exits when raw_rx is closed (all senders dropped).
#[tokio::test]
async fn exits_on_raw_channel_close() {
    let hub = Arc::new(Mutex::new(AgentHub::noop()));
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let (frontend_tx, _frontend_rx) = mpsc::channel(16);

    let handle = start_event_loop(hub, raw_rx, frontend_tx);

    drop(raw_tx);

    tokio::time::timeout(Duration::from_millis(200), handle)
        .await
        .expect("event loop should exit when raw channel closes")
        .expect("task should not panic");
}

/// Loop exits when frontend_tx is closed (receiver dropped).
#[tokio::test]
async fn exits_on_frontend_channel_close() {
    let hub = Arc::new(Mutex::new(AgentHub::noop()));
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let (frontend_tx, frontend_rx) = mpsc::channel(16);

    let handle = start_event_loop(hub, raw_rx, frontend_tx);
    drop(frontend_rx);

    // Send an event — the loop should detect frontend_tx.send() error and break.
    let event = AgentEvent::root(AgentEventPayload::Started);
    let _ = raw_tx.send(event).await;

    tokio::time::timeout(Duration::from_millis(200), handle)
        .await
        .expect("event loop should exit when frontend is gone")
        .expect("task should not panic");
}

/// SubAgentSpawned is forwarded to frontend AND triggers background attach.
/// Since there's no real TCP server, the attach will fail — but the event
/// must still reach the frontend.
#[tokio::test]
async fn sub_agent_spawned_forwarded_despite_attach_failure() {
    let hub = Arc::new(Mutex::new(AgentHub::noop()));
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let (frontend_tx, mut frontend_rx) = mpsc::channel(16);

    let _handle = start_event_loop(hub, raw_rx, frontend_tx);

    let event = AgentEvent::root(AgentEventPayload::SubAgentSpawned {
        name: "test-agent".into(),
        pid: 12345,
        port: 1, // invalid port — attach will fail
        token: "fake-token".into(),
    });
    raw_tx.send(event).await.unwrap();

    // Event should still be forwarded to frontend.
    let received = tokio::time::timeout(Duration::from_millis(100), frontend_rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    assert!(matches!(
        received.payload,
        AgentEventPayload::SubAgentSpawned { .. }
    ));
}
