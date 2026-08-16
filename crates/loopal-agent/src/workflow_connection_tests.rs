use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    MAX_WORKFLOW_WAIT_MS, WorkflowCancelRequest, WorkflowGetRequest, WorkflowRunId,
    WorkflowStartLookupRequest, WorkflowStartRequest, WorkflowWaitRequest, WorkflowWaitStatus,
};

use super::*;

fn pair() -> (
    ConnectionWorkflowControlClient,
    Arc<Connection<loopal_ipc::connection::Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (client_stream, hub_stream) = loopal_ipc::duplex_pair();
    let (client, _) = Connection::new(client_stream).into_listening();
    let (hub, incoming) = Connection::new(hub_stream).into_listening();
    (ConnectionWorkflowControlClient::new(client), hub, incoming)
}

fn start_request() -> WorkflowStartRequest {
    serde_json::from_value(serde_json::json!({
        "request_id": "wreq_start",
        "spec": {
            "version": 1,
            "run_goal": "test",
            "nodes": [{"id": "node", "dependencies": [], "task": "work", "worker_profile": "default"}],
            "limits": {"max_nodes": 1, "max_parallel": 1, "max_attempts": 1, "run_deadline_ms": 1000, "attempt_timeout_ms": 500, "max_output_bytes": 1024},
            "output_node": "node",
            "output_contract": {"type": "text", "max_bytes": 1024}
        }
    }))
    .unwrap()
}

#[tokio::test]
async fn start_roundtrip_keeps_invalid_success_response_indeterminate() {
    let (client, hub, mut incoming) = pair();
    let request = tokio::spawn(async move { client.start(start_request()).await });
    let Incoming::Request { id, method, .. } = incoming.recv().await.unwrap() else {
        panic!("expected workflow start request")
    };
    assert_eq!(method, methods::HUB_WORKFLOW_START.name);
    hub.respond(id, serde_json::json!({})).await.unwrap();
    assert!(matches!(
        request.await.unwrap(),
        Err(WorkflowStartControlError::Indeterminate { request_id, .. })
            if request_id.as_str() == "wreq_start"
    ));
}

#[tokio::test]
async fn non_start_methods_use_typed_rpc_and_wait_limit() {
    let (client, hub, mut incoming) = pair();
    let responder = tokio::spawn(async move {
        for (expected, response) in [
            (
                methods::HUB_WORKFLOW_GET.name,
                serde_json::json!({"run": null}),
            ),
            (
                methods::HUB_WORKFLOW_LOOKUP_START.name,
                serde_json::json!({"status": "not_found"}),
            ),
            (
                methods::HUB_WORKFLOW_WAIT.name,
                serde_json::json!({"status": "timed_out", "run": null}),
            ),
            (methods::HUB_WORKFLOW_CANCEL.name, serde_json::json!({})),
        ] {
            let Incoming::Request { id, method, .. } = incoming.recv().await.unwrap() else {
                panic!("expected workflow control request")
            };
            assert_eq!(method, expected);
            hub.respond(id, response).await.unwrap();
        }
    });
    let run_id = WorkflowRunId::new("wrun_test");
    assert!(
        client
            .get(WorkflowGetRequest {
                request_id: "wreq_get".into(),
                run_id: run_id.clone(),
            })
            .await
            .unwrap()
            .run
            .is_none()
    );
    assert!(matches!(
        client
            .lookup_start(WorkflowStartLookupRequest {
                request_id: "wreq_lookup".into(),
            })
            .await
            .unwrap(),
        WorkflowStartLookupResponse::NotFound
    ));
    let wait = client
        .wait(WorkflowWaitRequest {
            request_id: "wreq_wait".into(),
            run_id: run_id.clone(),
            after_revision: 0,
            timeout_ms: 10,
        })
        .await
        .unwrap();
    assert_eq!(wait.status, WorkflowWaitStatus::TimedOut);
    assert!(
        client
            .cancel(WorkflowCancelRequest {
                request_id: "wreq_cancel".into(),
                run_id: run_id.clone(),
                reason: None,
            })
            .await
            .unwrap_err()
            .contains("invalid response")
    );
    assert!(
        client
            .wait(WorkflowWaitRequest {
                request_id: "wreq_too_long".into(),
                run_id,
                after_revision: 0,
                timeout_ms: MAX_WORKFLOW_WAIT_MS + 1,
            })
            .await
            .unwrap_err()
            .contains("exceeds")
    );
    responder.await.unwrap();
}
