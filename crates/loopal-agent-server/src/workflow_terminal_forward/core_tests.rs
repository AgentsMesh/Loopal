use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_protocol::{
    InterruptSignal, WorkflowRunId, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalDisposition, WorkflowTerminalNotification, WorkflowTerminalOutcome,
};
use loopal_runtime::agent_input::AgentInput;

use super::forward_with_timeout;
use crate::session_hub::SharedSession;

pub(super) type Peer = (
    Arc<Connection<Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
);

pub(super) fn peers() -> (Peer, Peer) {
    let (left, right) = loopal_ipc::duplex_pair();
    (
        Connection::new(left).into_listening(),
        Connection::new(right).into_listening(),
    )
}

pub(super) fn session(id: &str) -> (Arc<SharedSession>, tokio::sync::mpsc::Receiver<AgentInput>) {
    let (input_tx, input_rx) = tokio::sync::mpsc::channel(4);
    let (interrupt_tx, _) = tokio::sync::watch::channel(0);
    (
        Arc::new(SharedSession::new(
            id.into(),
            input_tx,
            InterruptSignal::new(),
            Arc::new(interrupt_tx),
        )),
        input_rx,
    )
}

pub(super) fn notification(session_id: &str) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            session_id,
            WorkflowRunId::new("wrun_server_test"),
            2,
        ),
        state: WorkflowRunState::Succeeded,
        run_goal: "finish".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "done".into(),
        },
        content: "workflow done".into(),
    }
}

pub(super) async fn request_parts(
    client: Arc<Connection<Listening>>,
    incoming: &mut tokio::sync::mpsc::Receiver<Incoming>,
    notification: &WorkflowTerminalNotification,
) -> (
    tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>>,
    i64,
    serde_json::Value,
) {
    let params = serde_json::to_value(notification).unwrap();
    let request = tokio::spawn(async move { client.send_request("terminal-test", params).await });
    let Incoming::Request { id, params, .. } = incoming.recv().await.unwrap() else {
        panic!("expected request")
    };
    (request, id, params)
}

#[tokio::test]
async fn valid_request_enqueues_and_returns_runtime_disposition() {
    let ((server, mut incoming), (client, _)) = peers();
    let (session, mut input_rx) = session("session-server");
    let (response, id, params) =
        request_parts(client, &mut incoming, &notification("session-server")).await;

    tokio::join!(
        forward_with_timeout(id, params, &session, &server, Duration::from_secs(1)),
        async {
            let AgentInput::WorkflowTerminal(request) = input_rx.recv().await.unwrap() else {
                panic!("expected workflow terminal")
            };
            request
                .acknowledge(WorkflowTerminalDisposition::Applied)
                .await;
        }
    );
    let value = response.await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_value::<WorkflowTerminalDisposition>(value).unwrap(),
        WorkflowTerminalDisposition::Applied
    );
}

#[tokio::test]
async fn timeout_singleflights_duplicate_and_rejects_conflict() {
    let ((server, mut incoming), (client, _)) = peers();
    let (session, mut input_rx) = session("session-pending");
    let original = notification("session-pending");
    let (first, id, params) = request_parts(client.clone(), &mut incoming, &original).await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    assert_eq!(first.await.unwrap().unwrap()["status"], "queued");

    let (duplicate, id, params) = request_parts(client.clone(), &mut incoming, &original).await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    assert_eq!(duplicate.await.unwrap().unwrap()["status"], "queued");

    let mut conflict = original;
    conflict.content = "equivocating payload".into();
    let (conflict_response, id, params) =
        request_parts(client.clone(), &mut incoming, &conflict).await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    assert_eq!(
        conflict_response.await.unwrap().unwrap()["status"],
        "rejected"
    );

    let AgentInput::WorkflowTerminal(request) = input_rx.try_recv().unwrap() else {
        panic!("expected one queued workflow terminal")
    };
    assert!(input_rx.try_recv().is_err(), "duplicate must not enqueue");
    request
        .acknowledge(WorkflowTerminalDisposition::Applied)
        .await;
    drop(request);

    let original = notification("session-pending");
    let (cached, id, params) = request_parts(client, &mut incoming, &original).await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    assert_eq!(cached.await.unwrap().unwrap()["status"], "applied");
    assert!(
        input_rx.try_recv().is_err(),
        "cached result must not enqueue"
    );
}

#[tokio::test]
async fn mismatch_is_typed_rejection_and_closed_input_is_rpc_error() {
    let ((server, mut incoming), (client, _)) = peers();
    let (session, input_rx) = session("bound-session");
    let (mismatch, id, params) = request_parts(
        client.clone(),
        &mut incoming,
        &notification("other-session"),
    )
    .await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    assert_eq!(mismatch.await.unwrap().unwrap()["status"], "rejected");

    drop(input_rx);
    let (closed, id, params) =
        request_parts(client, &mut incoming, &notification("bound-session")).await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    let error = closed.await.unwrap().unwrap_err();
    assert_eq!(
        error.remote_code(),
        Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
    );
}

#[tokio::test]
async fn observer_cannot_inject_workflow_terminal() {
    let ((server, incoming), (client, _)) = peers();
    let (session, _input_rx) = session("observer-session");
    let observer = tokio::spawn(async move {
        let mut incoming = incoming;
        crate::session_forward::observer_loop(&mut incoming, &server, &session, "observer").await;
    });
    let error = client
        .send_request(
            loopal_ipc::protocol::methods::AGENT_WORKFLOW_TERMINAL.name,
            serde_json::to_value(notification("observer-session")).unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.remote_code(),
        Some(loopal_ipc::jsonrpc::METHOD_NOT_FOUND)
    );
    observer.abort();
}
