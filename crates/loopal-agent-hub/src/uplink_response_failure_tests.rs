use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use tokio::sync::{Mutex, mpsc};

use super::handle_reverse_requests;
use crate::Hub;

struct FailingResponseTransport {
    closed: AtomicBool,
}

#[async_trait]
impl Transport for FailingResponseTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        Err(loopal_error::LoopalError::Ipc("response failed".into()))
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

#[tokio::test]
async fn reverse_response_failure_closes_uplink_and_stops_loop() {
    let transport = Arc::new(FailingResponseTransport {
        closed: AtomicBool::new(false),
    });
    let (connection, _connection_rx) = Connection::new(transport.clone()).into_listening();
    let (incoming_tx, incoming_rx) = mpsc::channel(1);
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let handler = tokio::spawn(handle_reverse_requests(
        hub,
        connection,
        incoming_rx,
        "hub-a".into(),
    ));

    incoming_tx
        .send(Incoming::Request {
            id: 1,
            method: methods::AGENT_MESSAGE.name.into(),
            params: serde_json::json!({"bad": true}),
        })
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(1), handler)
        .await
        .expect("reverse loop should stop after response failure")
        .unwrap();
    assert!(transport.closed.load(Ordering::Acquire));
}
