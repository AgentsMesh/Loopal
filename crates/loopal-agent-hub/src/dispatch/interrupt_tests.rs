use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use loopal_ipc::{Connection, Transport};
use tokio::sync::{Mutex, mpsc};

use super::{handle_control, handle_interrupt, handle_shutdown_agent};
use crate::Hub;
use crate::pending_relay::PendingPlanApprovalInfo;

struct RecordingTransport {
    sent: mpsc::UnboundedSender<Vec<u8>>,
    incoming_tx: mpsc::UnboundedSender<Vec<u8>>,
    incoming_rx: Mutex<mpsc::UnboundedReceiver<Vec<u8>>>,
    fail_first_send: AtomicBool,
    closed: AtomicBool,
    interrupt_signaled: AtomicBool,
    response_before_signal: AtomicBool,
}

#[async_trait]
impl Transport for RecordingTransport {
    async fn send(&self, data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        self.sent
            .send(data.to_vec())
            .map_err(|error| loopal_error::LoopalError::Other(error.to_string()))?;
        if self.fail_first_send.swap(false, Ordering::SeqCst) {
            return Err(loopal_error::LoopalError::Other("send failed".into()));
        }
        let value: serde_json::Value = serde_json::from_slice(data)
            .map_err(|error| loopal_error::LoopalError::Other(error.to_string()))?;
        if value["method"] == "agent/interrupt" {
            self.interrupt_signaled.store(true, Ordering::SeqCst);
            let id = value["id"].as_i64().unwrap();
            self.incoming_tx
                .send(loopal_ipc::jsonrpc::encode_response(
                    id,
                    serde_json::json!({"ok": true}),
                ))
                .map_err(|error| loopal_error::LoopalError::Other(error.to_string()))?;
        } else if value["id"] == 42 && !self.interrupt_signaled.load(Ordering::SeqCst) {
            self.response_before_signal.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    async fn recv(&self) -> Result<Option<Vec<u8>>, loopal_error::LoopalError> {
        Ok(self.incoming_rx.lock().await.recv().await)
    }

    fn is_connected(&self) -> bool {
        !self.closed.load(Ordering::SeqCst)
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

async fn fixture(
    fail_first_send: bool,
) -> (
    Arc<Mutex<Hub>>,
    Arc<RecordingTransport>,
    mpsc::UnboundedReceiver<Vec<u8>>,
) {
    let (sent_tx, sent_rx) = mpsc::unbounded_channel();
    let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
    let transport = Arc::new(RecordingTransport {
        sent: sent_tx,
        incoming_tx,
        incoming_rx: Mutex::new(incoming_rx),
        fail_first_send: AtomicBool::new(fail_first_send),
        closed: AtomicBool::new(false),
        interrupt_signaled: AtomicBool::new(false),
        response_before_signal: AtomicBool::new(false),
    });
    let (conn, _incoming) = Connection::new(transport.clone()).into_listening();
    let (event_tx, event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(event_tx);
    hub.registry
        .register_connection("main", conn.clone())
        .unwrap();
    hub.pending_plan_approvals.insert(
        ("main".into(), "plan".into()),
        PendingPlanApprovalInfo {
            agent_conn: conn,
            agent_ipc_id: 42,
            agent_name: "main".into(),
            interaction_id: "interaction-plan".into(),
            logical_id: "plan".into(),
        },
    );
    let hub = Arc::new(Mutex::new(hub));
    let _event_loop = crate::start_event_loop(hub.clone(), event_rx);
    (hub, transport, sent_rx)
}

#[tokio::test]
async fn interrupt_ack_confirms_signal_before_pending_response() {
    let (hub, transport, mut sent) = fixture(false).await;
    handle_interrupt(&hub, serde_json::json!({"target": "main"}))
        .await
        .unwrap();

    let first = sent.recv().await.unwrap();
    let first: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(first["method"], "agent/interrupt");
    let second = tokio::time::timeout(Duration::from_secs(1), sent.recv())
        .await
        .unwrap()
        .unwrap();
    let second: serde_json::Value = serde_json::from_slice(&second).unwrap();
    assert_eq!(second["id"], 42);
    assert_eq!(second["result"]["reason"], "interrupted");
    assert!(transport.interrupt_signaled.load(Ordering::SeqCst));
    assert!(!transport.response_before_signal.load(Ordering::SeqCst));
}

#[tokio::test]
async fn failed_interrupt_request_still_cleans_pending_and_closes_transport() {
    let (hub, transport, _sent) = fixture(true).await;
    let result = handle_interrupt(&hub, serde_json::json!({"target": "main"})).await;
    assert!(result.is_err());
    assert!(transport.closed.load(Ordering::SeqCst));
    assert!(hub.lock().await.pending_plan_approvals.is_empty());
}

#[tokio::test]
async fn blackhole_control_times_out_and_closes_captured_connection() {
    let (hub, transport, _sent) = fixture(false).await;
    let error = handle_control(
        &hub,
        serde_json::json!({"target": "main", "command": {"type": "resume"}}),
    )
    .await
    .unwrap_err();
    assert!(error.contains("timed out"));
    assert!(transport.closed.load(Ordering::SeqCst));
}

#[tokio::test]
async fn blackhole_shutdown_times_out_and_closes_captured_connection() {
    let (hub, transport, _sent) = fixture(false).await;
    let error = handle_shutdown_agent(&hub, serde_json::json!({"target": "main"}))
        .await
        .unwrap_err();
    assert!(error.contains("timed out"));
    assert!(transport.closed.load(Ordering::SeqCst));
}
