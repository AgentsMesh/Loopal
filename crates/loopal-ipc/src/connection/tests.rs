use std::future::pending;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use tokio::sync::{Notify, mpsc};

use super::*;

struct BlockingFirstSendTransport {
    send_count: AtomicUsize,
    first_send_started: Notify,
    sent: mpsc::UnboundedSender<Vec<u8>>,
    closed: AtomicBool,
}

#[async_trait]
impl Transport for BlockingFirstSendTransport {
    async fn send(&self, data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        if self.send_count.fetch_add(1, Ordering::SeqCst) == 0 {
            self.first_send_started.notify_one();
            pending::<()>().await;
        }
        self.sent
            .send(data.to_vec())
            .map_err(|error| loopal_error::LoopalError::Other(error.to_string()))
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        pending().await
    }

    fn is_connected(&self) -> bool {
        true
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

#[tokio::test]
async fn abort_during_transport_send_cleans_pending_and_closes_transport() {
    let (sent_tx, mut sent_rx) = mpsc::unbounded_channel();
    let transport = Arc::new(BlockingFirstSendTransport {
        send_count: AtomicUsize::new(0),
        first_send_started: Notify::new(),
        sent: sent_tx,
        closed: AtomicBool::new(false),
    });
    let (connection, _incoming) = Connection::new(transport.clone()).into_listening();

    let request = tokio::spawn({
        let connection = connection.clone();
        async move {
            connection
                .send_request("agent/plan_approval", serde_json::json!({}))
                .await
        }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        transport.first_send_started.notified(),
    )
    .await
    .expect("request send should start");
    assert_eq!(connection.pending.lock().await.len(), 1);

    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());

    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !transport.closed.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("transport close should not time out");
    assert!(
        sent_rx.try_recv().is_err(),
        "must not write after a partial frame"
    );
    assert!(connection.pending.lock().await.is_empty());
}

#[tokio::test]
async fn notification_frame_write_timeout_closes_transport() {
    let (sent_tx, mut sent_rx) = mpsc::unbounded_channel();
    let transport = Arc::new(BlockingFirstSendTransport {
        send_count: AtomicUsize::new(0),
        first_send_started: Notify::new(),
        sent: sent_tx,
        closed: AtomicBool::new(false),
    });
    let (connection, _incoming) = Connection::new(transport.clone()).into_listening();

    let error = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        connection.send_notification("agent/event", serde_json::json!({})),
    )
    .await
    .expect("frame write deadline must be bounded")
    .unwrap_err();

    assert!(error.to_string().contains("frame write timed out"));
    assert!(transport.closed.load(Ordering::Acquire));
    assert!(sent_rx.try_recv().is_err());
}
