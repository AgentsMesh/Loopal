use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::transport::Transport;
use loopal_protocol::InterruptSignal;
use tokio::sync::Notify;

use super::*;

struct BlackholeTransport {
    send_started: Notify,
    closed: AtomicBool,
}

#[async_trait]
impl Transport for BlackholeTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        self.send_started.notify_one();
        pending().await
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        pending().await
    }

    fn is_connected(&self) -> bool {
        !self.closed.load(Ordering::Acquire)
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

fn session() -> Arc<SharedSession> {
    let (input_tx, _input_rx) = tokio::sync::mpsc::channel(1);
    let (interrupt_tx, _interrupt_rx) = tokio::sync::watch::channel(0);
    Arc::new(SharedSession::placeholder(
        input_tx,
        InterruptSignal::new(),
        Arc::new(interrupt_tx),
    ))
}

#[tokio::test(start_paused = true)]
async fn blackhole_observer_is_bounded_and_does_not_block_primary() {
    let session = session();
    let (primary_peer, primary_server) = loopal_ipc::duplex_pair();
    let (primary, _primary_incoming) = Connection::new(primary_server).into_listening();
    let (_peer, mut primary_rx) = Connection::new(primary_peer).into_listening();
    session.add_client("primary".into(), primary.clone()).await;

    let blackhole = Arc::new(BlackholeTransport {
        send_started: Notify::new(),
        closed: AtomicBool::new(false),
    });
    let (observer, _observer_rx) = Connection::new(blackhole.clone()).into_listening();
    session.add_client("observer".into(), observer).await;
    // Delivery must not depend on vector order. Keep the real primary flag on
    // its lease while placing the observer first to expose serial fan-out.
    session.clients.lock().await.swap(0, 1);

    let delivery = tokio::spawn({
        let session = session.clone();
        async move { deliver(&session, serde_json::json!({"event": "tool_result"})).await }
    });
    blackhole.send_started.notified().await;

    let primary_message = loop {
        match primary_rx.try_recv() {
            Ok(message) => break message,
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => tokio::task::yield_now().await,
            Err(error) => panic!("primary connection closed: {error}"),
        }
    };
    assert!(matches!(
        primary_message,
        Incoming::Notification { method, .. }
            if method == loopal_ipc::protocol::methods::AGENT_EVENT.name
    ));
    assert!(
        !delivery.is_finished(),
        "observer should still be at its deadline"
    );

    tokio::time::advance(EVENT_DELIVERY_DEADLINE).await;
    assert_eq!(delivery.await.unwrap(), Ok(()));

    assert!(blackhole.closed.load(Ordering::Acquire));
    assert_eq!(session.all_connections().await.len(), 1);
    assert!(Arc::ptr_eq(
        &session.primary_connection().await.unwrap(),
        &primary
    ));
}

#[tokio::test(start_paused = true)]
async fn blackhole_primary_reports_failure_after_delivering_to_observer() {
    let session = session();
    let blackhole = Arc::new(BlackholeTransport {
        send_started: Notify::new(),
        closed: AtomicBool::new(false),
    });
    let (primary, _primary_rx) = Connection::new(blackhole.clone()).into_listening();
    session.add_client("primary".into(), primary).await;

    let (observer_peer, observer_server) = loopal_ipc::duplex_pair();
    let (observer, _observer_incoming) = Connection::new(observer_server).into_listening();
    let (_peer, mut observer_rx) = Connection::new(observer_peer).into_listening();
    session
        .add_client("observer".into(), observer.clone())
        .await;

    let delivery = tokio::spawn({
        let session = session.clone();
        async move { deliver(&session, serde_json::json!({"event": "critical"})).await }
    });
    blackhole.send_started.notified().await;

    let observer_message = loop {
        match observer_rx.try_recv() {
            Ok(message) => break message,
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => tokio::task::yield_now().await,
            Err(error) => panic!("observer connection closed: {error}"),
        }
    };
    assert!(matches!(
        observer_message,
        Incoming::Notification { method, .. }
            if method == loopal_ipc::protocol::methods::AGENT_EVENT.name
    ));
    assert!(
        !delivery.is_finished(),
        "primary should still be at its deadline"
    );

    tokio::time::advance(EVENT_DELIVERY_DEADLINE).await;
    assert_eq!(
        delivery.await.unwrap(),
        Err(DeliveryError::PrimaryConnectionFailed {
            client_id: "primary".into(),
        })
    );

    assert!(blackhole.closed.load(Ordering::Acquire));
    assert_eq!(session.all_connections().await.len(), 1);
    assert!(Arc::ptr_eq(
        &session.primary_connection().await.unwrap(),
        &observer
    ));
}

#[tokio::test]
async fn no_connections_is_a_delivery_failure() {
    let session = session();
    assert_eq!(
        deliver(&session, serde_json::json!({"event": "critical"})).await,
        Err(DeliveryError::NoConnections)
    );
}

#[tokio::test(start_paused = true)]
async fn all_failed_connections_are_reported_after_cleanup() {
    let session = session();
    let blackhole = Arc::new(BlackholeTransport {
        send_started: Notify::new(),
        closed: AtomicBool::new(false),
    });
    let (primary, _primary_rx) = Connection::new(blackhole.clone()).into_listening();
    session.add_client("primary".into(), primary).await;

    let delivery = tokio::spawn({
        let session = session.clone();
        async move { deliver(&session, serde_json::json!({"event": "critical"})).await }
    });
    blackhole.send_started.notified().await;
    tokio::time::advance(EVENT_DELIVERY_DEADLINE).await;

    assert_eq!(
        delivery.await.unwrap(),
        Err(DeliveryError::AllConnectionsFailed { attempted: 1 })
    );
    assert!(session.all_connections().await.is_empty());
}
