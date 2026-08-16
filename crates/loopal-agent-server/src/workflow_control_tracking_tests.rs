use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_agent::workflow_control::{WorkflowControlClient, WorkflowStartControlError};
use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowRunId, WorkflowStartLookupRequest, WorkflowStartLookupResponse, WorkflowStartRequest,
    WorkflowStartResponse, WorkflowWaitRequest, WorkflowWaitResponse,
};

use super::TrackingWorkflowControlClient;

#[path = "workflow_control_tracking_support_tests.rs"]
mod support;
use support::{request, response, summary};

struct Stub {
    start: Mutex<VecDeque<Result<WorkflowStartResponse, WorkflowStartControlError>>>,
    lookup: Mutex<VecDeque<Result<WorkflowStartLookupResponse, String>>>,
    calls: Mutex<Vec<&'static str>>,
}

#[async_trait]
impl WorkflowControlClient for Stub {
    async fn start(
        &self,
        _: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        self.calls.lock().unwrap().push("start");
        self.start.lock().unwrap().pop_front().unwrap()
    }

    async fn lookup_start(
        &self,
        _: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, String> {
        self.calls.lock().unwrap().push("lookup");
        self.lookup
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(WorkflowStartLookupResponse::NotFound))
    }

    async fn get(&self, _: WorkflowGetRequest) -> Result<WorkflowGetResponse, String> {
        self.calls.lock().unwrap().push("get");
        Ok(WorkflowGetResponse { run: None })
    }

    async fn wait(&self, _: WorkflowWaitRequest) -> Result<WorkflowWaitResponse, String> {
        self.calls.lock().unwrap().push("wait");
        Ok(WorkflowWaitResponse {
            status: loopal_protocol::WorkflowWaitStatus::NotFound,
            run: None,
        })
    }

    async fn cancel(&self, _: WorkflowCancelRequest) -> Result<WorkflowCancelResponse, String> {
        self.calls.lock().unwrap().push("cancel");
        Ok(WorkflowCancelResponse {
            summary: summary("wrun_cancel"),
            already_terminal: true,
        })
    }
}

#[tokio::test]
async fn tracks_confirmed_starts_and_found_lookups() {
    let start_response = response("wrun_start");
    let lookup_response = response("wrun_lookup");
    let stub = Arc::new(Stub {
        start: Mutex::new(VecDeque::from([
            Ok(start_response.clone()),
            Err(WorkflowStartControlError::Rejected("denied".into())),
        ])),
        lookup: Mutex::new(VecDeque::from([
            Ok(WorkflowStartLookupResponse::Found {
                response: lookup_response,
            }),
            Ok(WorkflowStartLookupResponse::NotFound),
            Err("lookup failed".into()),
        ])),
        calls: Mutex::new(Vec::new()),
    });
    let leases = Arc::new(loopal_runtime::WorkflowLeaseTracker::default());
    let client = TrackingWorkflowControlClient::new(stub.clone(), leases.clone());

    let started = client.start(request("wreq_start")).await.unwrap();
    assert_eq!(started.summary.id, WorkflowRunId::new("wrun_start"));
    assert!(leases.has_outstanding());

    let rejected = client.start(request("wreq_rejected")).await.unwrap_err();
    assert_eq!(
        rejected,
        WorkflowStartControlError::Rejected("denied".into())
    );

    let found = client
        .lookup_start(WorkflowStartLookupRequest {
            request_id: "wreq_lookup".into(),
        })
        .await
        .unwrap();
    assert!(matches!(found, WorkflowStartLookupResponse::Found { .. }));

    let not_found = client
        .lookup_start(WorkflowStartLookupRequest {
            request_id: "wreq_none".into(),
        })
        .await
        .unwrap();
    assert_eq!(not_found, WorkflowStartLookupResponse::NotFound);
    assert!(
        client
            .lookup_start(WorkflowStartLookupRequest {
                request_id: "wreq_error".into(),
            })
            .await
            .is_err()
    );
    assert_eq!(
        stub.calls.lock().unwrap().as_slice(),
        ["start", "start", "lookup", "lookup", "lookup"]
    );
}

#[tokio::test]
async fn delegates_non_start_operations_without_tracking() {
    let stub = Arc::new(Stub {
        start: Mutex::new(VecDeque::new()),
        lookup: Mutex::new(VecDeque::new()),
        calls: Mutex::new(Vec::new()),
    });
    let leases = Arc::new(loopal_runtime::WorkflowLeaseTracker::default());
    let client = TrackingWorkflowControlClient::new(stub.clone(), leases.clone());
    assert!(
        client
            .get(WorkflowGetRequest {
                request_id: "wreq_get".into(),
                run_id: "wrun_get".into(),
            })
            .await
            .unwrap()
            .run
            .is_none()
    );
    assert_eq!(
        client
            .wait(WorkflowWaitRequest {
                request_id: "wreq_wait".into(),
                run_id: "wrun_wait".into(),
                after_revision: 0,
                timeout_ms: 1,
            })
            .await
            .unwrap()
            .status,
        loopal_protocol::WorkflowWaitStatus::NotFound
    );
    assert!(
        client
            .cancel(WorkflowCancelRequest {
                request_id: "wreq_cancel".into(),
                run_id: "wrun_cancel".into(),
                reason: None,
            })
            .await
            .unwrap()
            .already_terminal
    );
    assert!(!leases.has_outstanding());
    assert_eq!(
        stub.calls.lock().unwrap().as_slice(),
        ["get", "wait", "cancel"]
    );
}
