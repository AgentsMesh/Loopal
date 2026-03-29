//! Tests for the hub event router.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::{Hub, start_event_loop};
use loopal_protocol::{AgentEvent, AgentEventPayload};

fn make_hub_and_channels() -> (
    Arc<Mutex<Hub>>,
    mpsc::Sender<AgentEvent>,
    mpsc::Receiver<AgentEvent>,
) {
    let (raw_tx, raw_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(raw_tx.clone())));
    (hub, raw_tx, raw_rx)
}

/// Normal events are forwarded via broadcast.
#[tokio::test]
async fn forwards_events_to_subscriber() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let mut sub_rx = hub.lock().await.ui.subscribe_events();

    let _handle = start_event_loop(hub, raw_rx);

    let event = AgentEvent::root(AgentEventPayload::Stream {
        text: "hello".into(),
    });
    raw_tx.send(event).await.unwrap();

    let received = tokio::time::timeout(Duration::from_millis(100), sub_rx.recv())
        .await
        .expect("timeout")
        .expect("recv error");

    assert!(matches!(received.payload, AgentEventPayload::Stream { .. }));
}

/// Multiple events arrive in order.
#[tokio::test]
async fn preserves_event_order() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let mut sub_rx = hub.lock().await.ui.subscribe_events();

    let _handle = start_event_loop(hub, raw_rx);

    for i in 0..5 {
        let event = AgentEvent::root(AgentEventPayload::Stream {
            text: format!("msg-{i}"),
        });
        raw_tx.send(event).await.unwrap();
    }

    for i in 0..5 {
        let received = tokio::time::timeout(Duration::from_millis(100), sub_rx.recv())
            .await
            .expect("timeout")
            .expect("recv error");
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
    let hub = Arc::new(Mutex::new(Hub::noop()));
    let (raw_tx, raw_rx) = mpsc::channel::<AgentEvent>(16);

    let handle = start_event_loop(hub, raw_rx);
    drop(raw_tx);

    tokio::time::timeout(Duration::from_millis(200), handle)
        .await
        .expect("event loop should exit when raw channel closes")
        .expect("task should not panic");
}

/// Multiple subscribers each receive the same events.
#[tokio::test]
async fn multiple_subscribers_receive_events() {
    let (hub, raw_tx, raw_rx) = make_hub_and_channels();
    let mut sub1 = hub.lock().await.ui.subscribe_events();
    let mut sub2 = hub.lock().await.ui.subscribe_events();

    let _handle = start_event_loop(hub, raw_rx);

    let event = AgentEvent::root(AgentEventPayload::Stream {
        text: "broadcast".into(),
    });
    raw_tx.send(event).await.unwrap();

    let r1 = tokio::time::timeout(Duration::from_millis(100), sub1.recv())
        .await
        .expect("timeout")
        .expect("recv error");
    let r2 = tokio::time::timeout(Duration::from_millis(100), sub2.recv())
        .await
        .expect("timeout")
        .expect("recv error");

    assert!(matches!(r1.payload, AgentEventPayload::Stream { .. }));
    assert!(matches!(r2.payload, AgentEventPayload::Stream { .. }));
}
