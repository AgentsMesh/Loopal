use super::*;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{
    WorkflowAttemptCapability, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};

fn worker_start() -> StartParams {
    StartParams {
        workflow_permission_causation: Some(WorkflowPermissionCausation {
            run_id: WorkflowRunId::new("wrun_startup"),
            node_id: WorkflowNodeId::new("wnode_startup"),
            attempt_id: WorkflowAttemptId::new("watt_startup"),
        }),
        workflow_attempt_capability: Some(
            WorkflowAttemptCapability::parse("11".repeat(32)).unwrap(),
        ),
        ..StartParams::default()
    }
}

fn response(
    disposition: WorkflowWorkerHandshakeDisposition,
    attempt_state: WorkflowAttemptState,
) -> WorkflowWorkerHandshakeResponse {
    WorkflowWorkerHandshakeResponse {
        disposition,
        attempt_state,
    }
}

#[test]
fn accepts_only_live_states_consistent_with_the_disposition() {
    for value in [
        response(
            WorkflowWorkerHandshakeDisposition::Fresh,
            WorkflowAttemptState::Dispatching,
        ),
        response(
            WorkflowWorkerHandshakeDisposition::Fresh,
            WorkflowAttemptState::Running,
        ),
        response(
            WorkflowWorkerHandshakeDisposition::Recovered,
            WorkflowAttemptState::Running,
        ),
    ] {
        validate_response(value).unwrap();
    }
    for state in [
        WorkflowAttemptState::Succeeded,
        WorkflowAttemptState::Failed,
        WorkflowAttemptState::Cancelled,
    ] {
        assert!(
            validate_response(response(WorkflowWorkerHandshakeDisposition::Fresh, state)).is_err()
        );
    }
    assert!(
        validate_response(response(
            WorkflowWorkerHandshakeDisposition::Recovered,
            WorkflowAttemptState::Dispatching,
        ))
        .is_err()
    );
}

#[tokio::test]
async fn sends_handshake_over_the_authenticated_connection() {
    let (worker_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (worker, _worker_rx) = Connection::new(worker_transport).into_listening();
    let (hub, mut hub_rx) = Connection::new(hub_transport).into_listening();
    let worker_for_task = worker.clone();
    let start = worker_start();
    let worker_task = tokio::spawn(async move { send_if_worker(&worker_for_task, &start).await });

    let Incoming::Request { id, method, params } = hub_rx.recv().await.unwrap() else {
        panic!("expected worker handshake request");
    };
    assert_eq!(method, methods::HUB_WORKFLOW_WORKER_HANDSHAKE.name);
    let request: WorkflowWorkerHandshakeRequest = serde_json::from_value(params).unwrap();
    assert_eq!(
        request.causation.node_id,
        WorkflowNodeId::new("wnode_startup")
    );
    assert_eq!(request.capability.expose(), "11".repeat(32));
    hub.respond(
        id,
        serde_json::to_value(response(
            WorkflowWorkerHandshakeDisposition::Fresh,
            WorkflowAttemptState::Dispatching,
        ))
        .unwrap(),
    )
    .await
    .unwrap();
    worker_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn rejects_hub_handshake_errors_before_startup() {
    let (worker_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (worker, _worker_rx) = Connection::new(worker_transport).into_listening();
    let (hub, mut hub_rx) = Connection::new(hub_transport).into_listening();
    let worker_for_task = worker.clone();
    let start = worker_start();
    let worker_task = tokio::spawn(async move { send_if_worker(&worker_for_task, &start).await });
    let Incoming::Request { id, .. } = hub_rx.recv().await.unwrap() else {
        panic!("expected worker handshake request");
    };
    hub.respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, "stale worker")
        .await
        .unwrap();
    let error = worker_task
        .await
        .unwrap()
        .expect_err("rejection must fail startup");
    assert!(error.to_string().contains("rejected worker startup"));
}

#[tokio::test]
async fn no_proof_is_a_noop_but_partial_proof_fails_closed() {
    let (worker_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (worker, _worker_rx) = Connection::new(worker_transport).into_listening();
    let (_hub, mut hub_rx) = Connection::new(hub_transport).into_listening();

    send_if_worker(&worker, &StartParams::default())
        .await
        .unwrap();
    assert!(hub_rx.try_recv().is_err());

    let mut partial = worker_start();
    partial.workflow_attempt_capability = None;
    let error = send_if_worker(&worker, &partial)
        .await
        .expect_err("partial workflow proof must fail startup");
    assert!(error.to_string().contains("must be supplied together"));
    assert!(hub_rx.try_recv().is_err());
}
