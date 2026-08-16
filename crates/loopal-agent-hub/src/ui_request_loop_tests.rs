use std::future::pending;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::{Dispatcher, DispatcherBuilder, RpcError, Transport};
use loopal_protocol::UiCapabilities;
use serde_json::json;
use tokio::sync::{Mutex, Semaphore, mpsc};
use tokio::task::JoinHandle;

use super::{UI_CONTROL_REQUEST_LIMIT, UI_DATA_REQUEST_LIMIT, ui_client_io_loop};
use crate::Hub;

#[path = "ui_request_loop_tests/admission.rs"]
mod admission;
#[path = "ui_request_loop_tests/cancellation.rs"]
mod cancellation;
#[path = "ui_request_loop_tests/concurrency.rs"]
mod concurrency;
#[path = "ui_request_loop_tests/recovery.rs"]
mod recovery;
#[path = "ui_request_loop_tests/response_transport.rs"]
mod response_transport;

struct TestTransport {
    fail_send: bool,
    block_close: bool,
    closed: AtomicBool,
    frames: std::sync::Mutex<Vec<Vec<u8>>>,
    frame_sent: Semaphore,
}

impl TestTransport {
    fn recording() -> Arc<Self> {
        Self::new(false, false)
    }

    fn failing() -> Arc<Self> {
        Self::new(true, false)
    }

    fn blocking_close() -> Arc<Self> {
        Self::new(false, true)
    }

    fn new(fail_send: bool, block_close: bool) -> Arc<Self> {
        Arc::new(Self {
            fail_send,
            block_close,
            closed: AtomicBool::new(false),
            frames: std::sync::Mutex::new(Vec::new()),
            frame_sent: Semaphore::new(0),
        })
    }

    async fn wait_for_frame(&self) {
        tokio::time::timeout(Duration::from_secs(1), self.frame_sent.acquire())
            .await
            .expect("response frame should be sent")
            .unwrap()
            .forget();
    }

    fn frame(&self, index: usize) -> serde_json::Value {
        serde_json::from_slice(&self.frames.lock().unwrap()[index]).unwrap()
    }
}

#[async_trait]
impl Transport for TestTransport {
    async fn send(&self, data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        if self.fail_send {
            return Err(loopal_error::LoopalError::Other("send failed".into()));
        }
        self.frames.lock().unwrap().push(data.to_vec());
        self.frame_sent.add_permits(1);
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        pending().await
    }

    fn is_connected(&self) -> bool {
        !self.closed.load(Ordering::Acquire)
    }

    async fn close(&self) {
        if self.block_close {
            pending::<()>().await;
        }
        self.closed.store(true, Ordering::Release);
    }
}

fn test_connection(transport: Arc<TestTransport>) -> Arc<Connection<Listening>> {
    Connection::new(transport).into_listening().0
}

fn start_with_incoming(
    dispatcher: Dispatcher,
    transport: Arc<TestTransport>,
) -> (mpsc::Sender<Incoming>, JoinHandle<()>) {
    let server = test_connection(transport);
    let (tx, rx) = mpsc::channel(64);
    let mut hub_state = Hub::noop();
    hub_state.ui.register_client_with_lease(
        "ui-test",
        "ui-test",
        server.clone(),
        UiCapabilities::ALL,
    );
    let task = tokio::spawn(ui_client_io_loop(
        Arc::new(Mutex::new(hub_state)),
        Arc::new(dispatcher),
        server,
        rx,
        "ui-test".into(),
    ));
    (tx, task)
}

fn start(dispatcher: Dispatcher) -> (Arc<Connection<Listening>>, JoinHandle<()>) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (server, server_rx) = Connection::new(server_transport).into_listening();
    let mut hub_state = Hub::noop();
    hub_state.ui.register_client_with_lease(
        "ui-test",
        "ui-test",
        server.clone(),
        UiCapabilities::ALL,
    );
    let hub = Arc::new(Mutex::new(hub_state));
    let task = tokio::spawn(ui_client_io_loop(
        hub,
        Arc::new(dispatcher),
        server,
        server_rx,
        "ui-test".into(),
    ));
    (client, task)
}
