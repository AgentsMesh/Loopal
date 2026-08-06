use std::future::pending;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use loopal_ipc::Transport;

use super::*;

struct BlockedTransport {
    block_send: bool,
    closed: AtomicBool,
    sent: std::sync::Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl Transport for BlockedTransport {
    async fn send(&self, _data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        if self.block_send {
            pending().await
        } else {
            self.sent.lock().unwrap().push(_data.to_vec());
            Ok(())
        }
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

fn connection(block_send: bool) -> (Arc<Connection<Listening>>, Arc<BlockedTransport>) {
    let transport = Arc::new(BlockedTransport {
        block_send,
        closed: AtomicBool::new(false),
        sent: std::sync::Mutex::new(Vec::new()),
    });
    let (connection, _incoming) = Connection::new(transport.clone()).into_listening();
    (connection, transport)
}

#[tokio::test]
async fn registration_wait_is_bounded() {
    let (connection, transport) = connection(false);
    let (_tx, mut incoming) = tokio::sync::mpsc::channel(1);

    let result = receive_register(&connection, &mut incoming, "token").await;

    let error = result.err().expect("registration wait should fail");
    assert!(error.contains("timed out"));
    assert!(transport.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn registration_ack_is_bounded_before_ui_lease_activation() {
    let (connection, transport) = connection(true);
    let result = register::RegisterResult {
        request_id: 1,
        name: "blocked-ui".into(),
        role: ClientRole::UiClient,
        capabilities: loopal_protocol::UiCapabilities::ALL,
        lease_id: "lease".into(),
    };

    assert!(
        acknowledge_register(&connection, &result)
            .await
            .unwrap_err()
            .contains("timed out")
    );
    assert!(transport.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn duplicate_tcp_agent_is_rejected_before_success_ack() {
    let hub = Arc::new(tokio::sync::Mutex::new(Hub::noop()));
    let (existing, _) = connection(false);
    hub.lock()
        .await
        .registry
        .register_connection("worker", existing)
        .unwrap();
    let (candidate, transport) = connection(false);
    let (_incoming_tx, incoming) = tokio::sync::mpsc::channel(1);
    let result = register::RegisterResult {
        request_id: 9,
        name: "worker".into(),
        role: ClientRole::Agent,
        capabilities: loopal_protocol::UiCapabilities::NONE,
        lease_id: "agent-lease".into(),
    };

    let error = agent_registration::reserve_ack_and_start(hub, candidate, incoming, result)
        .await
        .unwrap_err();

    assert!(error.contains("already registered"));
    assert!(transport.closed.load(Ordering::Acquire));
    let frames = transport.sent.lock().unwrap();
    assert_eq!(frames.len(), 1);
    let response: serde_json::Value = serde_json::from_slice(&frames[0]).unwrap();
    assert!(response.get("error").is_some());
    assert!(response.get("result").is_none());
}

#[tokio::test]
async fn tcp_agent_is_not_routable_until_registration_ack_succeeds() {
    let hub = Arc::new(tokio::sync::Mutex::new(Hub::noop()));
    let (candidate, transport) = connection(true);
    let (_incoming_tx, incoming) = tokio::sync::mpsc::channel(1);
    let result = register::RegisterResult {
        request_id: 11,
        name: "reserved-worker".into(),
        role: ClientRole::Agent,
        capabilities: loopal_protocol::UiCapabilities::NONE,
        lease_id: "agent-lease".into(),
    };
    let registration_hub = hub.clone();
    let registration = tokio::spawn(async move {
        agent_registration::reserve_ack_and_start(registration_hub, candidate, incoming, result)
            .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("reserved-worker")
            .is_none()
    );
    assert!(registration.await.unwrap().is_err());
    assert!(transport.closed.load(Ordering::Acquire));

    let (replacement, _) = connection(false);
    hub.lock()
        .await
        .registry
        .register_connection("reserved-worker", replacement)
        .expect("failed ACK reservation must be reusable");
}

#[tokio::test]
async fn successful_tcp_agent_ack_activates_exact_reserved_connection() {
    let hub = Arc::new(tokio::sync::Mutex::new(Hub::noop()));
    let (candidate, transport) = connection(false);
    let (incoming_tx, incoming) = tokio::sync::mpsc::channel(1);
    let result = register::RegisterResult {
        request_id: 13,
        name: "accepted-worker".into(),
        role: ClientRole::Agent,
        capabilities: loopal_protocol::UiCapabilities::NONE,
        lease_id: "agent-lease".into(),
    };

    agent_registration::reserve_ack_and_start(hub.clone(), candidate.clone(), incoming, result)
        .await
        .unwrap();

    let active = hub
        .lock()
        .await
        .registry
        .get_agent_connection("accepted-worker")
        .unwrap();
    assert!(Arc::ptr_eq(&active, &candidate));
    {
        let frames = transport.sent.lock().unwrap();
        assert_eq!(frames.len(), 1);
        let response: serde_json::Value = serde_json::from_slice(&frames[0]).unwrap();
        assert_eq!(response["result"]["ok"], true);
    }

    drop(incoming_tx);
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if hub
                .lock()
                .await
                .registry
                .get_agent_connection("accepted-worker")
                .is_none()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("registered IO owner must clean up on input EOF");
}
