use std::future::pending;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use loopal_ipc::Transport;
use loopal_protocol::{InterruptSignal, PermissionIntentRequest};
use loopal_runtime::frontend::permission_handler::PermissionHandler;
use loopal_runtime::frontend::question_handler::{AskOptions, QuestionHandler};
use loopal_runtime::{PlanApproval, PlanApprovalCancellationReason};
use loopal_tool_api::PermissionDecision;
use tokio::sync::mpsc;

use super::plan::request_plan_approval_with_timeout;
use super::*;

const TEST_TIMEOUT: Duration = Duration::from_millis(20);

struct PendingSendTransport {
    send_count: AtomicUsize,
    sent: mpsc::UnboundedSender<Vec<u8>>,
    closed: AtomicBool,
}

#[async_trait]
impl Transport for PendingSendTransport {
    async fn send(&self, data: &[u8]) -> Result<(), loopal_error::LoopalError> {
        if self.send_count.fetch_add(1, Ordering::SeqCst) == 0 {
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

async fn pending_session() -> (SessionRef, Arc<PendingSendTransport>) {
    let (sent, sent_rx) = mpsc::unbounded_channel();
    let transport = Arc::new(PendingSendTransport {
        send_count: AtomicUsize::new(0),
        sent,
        closed: AtomicBool::new(false),
    });
    let (connection, _incoming) = Connection::new(transport.clone()).into_listening();
    let (input_tx, _input_rx) = mpsc::channel(1);
    let (interrupt_tx, _interrupt_rx) = tokio::sync::watch::channel(0);
    let session = Arc::new(SharedSession::placeholder(
        input_tx,
        InterruptSignal::new(),
        Arc::new(interrupt_tx),
    ));
    session.add_client("primary".into(), connection).await;
    drop(sent_rx);
    (Arc::new(tokio::sync::RwLock::new(session)), transport)
}

async fn assert_incomplete_transport_was_closed(transport: &PendingSendTransport) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !transport.closed.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("incomplete transport should close");
    assert_eq!(
        transport.send_count.load(Ordering::Acquire),
        1,
        "must not write a cancellation after a partial request frame"
    );
}

#[tokio::test]
async fn permission_send_timeout_denies_and_cancels_peer() {
    let (session, transport) = pending_session().await;
    let handler = IpcPermissionHandler::with_timeout(session, TEST_TIMEOUT);
    let request = PermissionIntentRequest::create(
        "tool-1",
        "Write",
        serde_json::json!({}),
        serde_json::json!({}),
        serde_json::json!({"type": "object"}),
        None,
    )
    .unwrap();

    let outcome = handler.decide(&request).await;

    assert_eq!(outcome.decision, PermissionDecision::Deny);
    assert!(outcome.reason.contains("timed out"));
    assert_incomplete_transport_was_closed(&transport).await;
}

#[tokio::test]
async fn question_send_timeout_cancels_with_logical_id_and_peer_notification() {
    let (session, transport) = pending_session().await;
    let handler = IpcQuestionHandler::with_timeout(session, TEST_TIMEOUT);

    let outcome = handler
        .ask_with_options(Vec::new(), AskOptions::manual("question-1"))
        .await;

    assert_eq!(
        outcome.response,
        UserQuestionResponse::cancelled("question-1")
    );
    assert!(outcome.reason.contains("timed out"));
    assert_incomplete_transport_was_closed(&transport).await;
}

#[tokio::test]
async fn plan_send_timeout_returns_timed_out_and_cancels_peer() {
    let (session, transport) = pending_session().await;

    let outcome =
        request_plan_approval_with_timeout(&session, "plan", "plan.md", TEST_TIMEOUT).await;

    assert_eq!(
        outcome,
        PlanApproval::Cancelled(PlanApprovalCancellationReason::TimedOut)
    );
    assert_incomplete_transport_was_closed(&transport).await;
}
