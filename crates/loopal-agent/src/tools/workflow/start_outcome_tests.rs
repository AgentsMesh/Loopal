use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowStartRequest, WorkflowStartResponse, WorkflowWaitRequest, WorkflowWaitResponse,
};
use loopal_tool_api::Tool;

use super::{context, run, sample_start};
use crate::workflow_control::{WorkflowControlClient, WorkflowStartControlError};

struct ScriptedStartClient {
    results: Mutex<VecDeque<Result<WorkflowStartResponse, WorkflowStartControlError>>>,
    requests: Mutex<Vec<WorkflowStartRequest>>,
}

#[async_trait]
impl WorkflowControlClient for ScriptedStartClient {
    async fn start(
        &self,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        self.requests.lock().unwrap().push(request);
        self.results.lock().unwrap().pop_front().unwrap()
    }

    async fn get(&self, _: WorkflowGetRequest) -> Result<WorkflowGetResponse, String> {
        unreachable!()
    }

    async fn wait(&self, _: WorkflowWaitRequest) -> Result<WorkflowWaitResponse, String> {
        unreachable!()
    }

    async fn cancel(&self, _: WorkflowCancelRequest) -> Result<WorkflowCancelResponse, String> {
        unreachable!()
    }
}

#[tokio::test]
async fn indeterminate_tool_start_confirms_with_the_exact_request() {
    let response = WorkflowStartResponse {
        summary: (&run()).into(),
    };
    let client = Arc::new(ScriptedStartClient {
        results: Mutex::new(VecDeque::from([
            Err(WorkflowStartControlError::Indeterminate {
                request_id: "wreq_start".into(),
                message: "response lost".into(),
            }),
            Ok(response),
        ])),
        requests: Mutex::new(Vec::new()),
    });

    let result = super::super::start::WorkflowStartTool
        .execute(sample_start(), &context(Some(client.clone())))
        .await
        .unwrap();

    assert!(!result.is_error);
    let requests = client.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[0].request_id.as_str(), "wreq_start");
}

#[tokio::test]
async fn still_indeterminate_tool_start_returns_the_stable_request_id() {
    let ambiguous = || WorkflowStartControlError::Indeterminate {
        request_id: "wreq_start".into(),
        message: "response lost".into(),
    };
    let client = Arc::new(ScriptedStartClient {
        results: Mutex::new(VecDeque::from([Err(ambiguous()), Err(ambiguous())])),
        requests: Mutex::new(Vec::new()),
    });

    let result = super::super::start::WorkflowStartTool
        .execute(sample_start(), &context(Some(client.clone())))
        .await
        .unwrap();

    assert!(result.is_error);
    let outcome: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(outcome["outcome"], "indeterminate");
    assert_eq!(outcome["request_id"], "wreq_start");
    let requests = client.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
}

#[tokio::test]
async fn rejection_after_ambiguity_does_not_claim_the_start_was_rejected() {
    let client = Arc::new(ScriptedStartClient {
        results: Mutex::new(VecDeque::from([
            Err(WorkflowStartControlError::Indeterminate {
                request_id: "wreq_start".into(),
                message: "response lost".into(),
            }),
            Err(WorkflowStartControlError::Rejected("lease changed".into())),
        ])),
        requests: Mutex::new(Vec::new()),
    });

    let result = super::super::start::WorkflowStartTool
        .execute(sample_start(), &context(Some(client.clone())))
        .await
        .unwrap();

    assert!(result.is_error);
    let outcome: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(outcome["outcome"], "indeterminate");
    assert_eq!(outcome["request_id"], "wreq_start");
    assert!(
        outcome["message"]
            .as_str()
            .unwrap()
            .contains("lease changed")
    );
    let requests = client.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
}
