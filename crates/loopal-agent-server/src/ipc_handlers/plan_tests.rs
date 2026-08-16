use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::InterruptSignal;
use loopal_runtime::{PlanApproval, PlanApprovalCancellationReason};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::plan::{
    default_plan_approval_timeout, request_plan_approval, request_plan_approval_with_timeout,
};
use super::*;

const PLAN_TIMEOUT: Duration = Duration::from_secs(1);

#[test]
fn public_plan_approval_uses_protocol_timeout() {
    assert_eq!(
        default_plan_approval_timeout(),
        loopal_protocol::DEFAULT_INTERACTION_RPC_TIMEOUT
    );
}

async fn session_pair() -> (
    SessionRef,
    Arc<Connection<Listening>>,
    mpsc::Receiver<Incoming>,
) {
    let (agent_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (peer, peer_rx) = Connection::new(peer_transport).into_listening();
    let (input_tx, _input_rx) = mpsc::channel(1);
    let (interrupt_tx, _interrupt_rx) = tokio::sync::watch::channel(0);
    let session = Arc::new(SharedSession::placeholder(
        input_tx,
        InterruptSignal::new(),
        Arc::new(interrupt_tx),
    ));
    session.add_client("primary".into(), agent).await;
    (Arc::new(tokio::sync::RwLock::new(session)), peer, peer_rx)
}

async fn respond(response: Value) -> (PlanApproval, Value) {
    let (session, peer, mut incoming) = session_pair().await;
    let task = tokio::spawn(async move {
        request_plan_approval_with_timeout(&session, "# Plan", "/tmp/plan.md", PLAN_TIMEOUT).await
    });
    let Incoming::Request { id, method, params } = incoming.recv().await.unwrap() else {
        panic!("expected plan approval request")
    };
    assert_eq!(method, methods::AGENT_PLAN_APPROVAL.name);
    peer.respond(id, response).await.unwrap();
    (task.await.unwrap(), params)
}

#[tokio::test]
async fn public_plan_approval_forwards_to_request_flow() {
    let (session, peer, mut incoming) = session_pair().await;
    let task =
        tokio::spawn(async move { request_plan_approval(&session, "plan", "plan.md").await });
    let Incoming::Request { id, .. } = incoming.recv().await.unwrap() else {
        panic!("expected plan approval request")
    };
    peer.respond(id, json!({"decision": "approve"}))
        .await
        .unwrap();
    assert_eq!(task.await.unwrap(), PlanApproval::Approve);
}

#[tokio::test]
async fn missing_primary_connection_is_transport_cancellation() {
    let (input_tx, _input_rx) = mpsc::channel(1);
    let (interrupt_tx, _interrupt_rx) = tokio::sync::watch::channel(0);
    let session = Arc::new(SharedSession::placeholder(
        input_tx,
        InterruptSignal::new(),
        Arc::new(interrupt_tx),
    ));
    let session = Arc::new(tokio::sync::RwLock::new(session));

    assert_eq!(
        request_plan_approval_with_timeout(&session, "plan", "plan.md", PLAN_TIMEOUT).await,
        PlanApproval::Cancelled(PlanApprovalCancellationReason::Transport)
    );
}

#[tokio::test]
async fn approve_and_edit_variants_preserve_request_payload() {
    let (approval, params) = respond(json!({"decision": "approve"})).await;
    assert_eq!(approval, PlanApproval::Approve);
    assert_eq!(params["plan_content"], "# Plan");
    assert_eq!(params["plan_path"], "/tmp/plan.md");
    assert!(Uuid::parse_str(params["request_id"].as_str().unwrap()).is_ok());

    let (approval, _) = respond(json!({
        "decision": "approve_with_edits",
        "edited_plan": "# Edited"
    }))
    .await;
    assert_eq!(approval, PlanApproval::ApproveWithEdits("# Edited".into()));
    let (approval, _) = respond(json!({"decision": "approve_with_edits"})).await;
    assert_eq!(approval, PlanApproval::Reject);
    let (approval, _) = respond(json!({"decision": "unknown"})).await;
    assert_eq!(approval, PlanApproval::Reject);
}

#[tokio::test]
async fn cancelled_reasons_map_all_wire_variants() {
    for (reason, expected) in [
        ("interrupted", PlanApprovalCancellationReason::Interrupted),
        ("timed_out", PlanApprovalCancellationReason::TimedOut),
        ("superseded", PlanApprovalCancellationReason::Superseded),
        ("transport", PlanApprovalCancellationReason::Transport),
        ("unknown", PlanApprovalCancellationReason::Unavailable),
    ] {
        let (approval, _) = respond(json!({
            "decision": "cancelled",
            "reason": reason
        }))
        .await;
        assert_eq!(approval, PlanApproval::Cancelled(expected));
    }
    let (approval, _) = respond(json!({"decision": "cancelled"})).await;
    assert_eq!(
        approval,
        PlanApproval::Cancelled(PlanApprovalCancellationReason::Unavailable)
    );
}

#[tokio::test]
async fn rpc_error_is_transport_cancellation() {
    let (session, peer, mut incoming) = session_pair().await;
    let task = tokio::spawn(async move {
        request_plan_approval_with_timeout(&session, "plan", "plan.md", PLAN_TIMEOUT).await
    });
    let Incoming::Request { id, .. } = incoming.recv().await.unwrap() else {
        panic!("expected plan approval request")
    };
    peer.respond_error(id, -32603, "unavailable").await.unwrap();
    assert_eq!(
        task.await.unwrap(),
        PlanApproval::Cancelled(PlanApprovalCancellationReason::Transport)
    );
}
