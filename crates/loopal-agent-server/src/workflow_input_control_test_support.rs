use std::collections::VecDeque;

use async_trait::async_trait;
use loopal_agent::workflow_control::{WorkflowControlClient, WorkflowStartControlError};
use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowStartLookupRequest, WorkflowStartLookupResponse, WorkflowStartRequest,
    WorkflowStartResponse, WorkflowWaitRequest, WorkflowWaitResponse,
};
use tokio::sync::Mutex;

pub(crate) struct ControlStub {
    pub(crate) results: Mutex<VecDeque<Result<WorkflowStartResponse, WorkflowStartControlError>>>,
    pub(crate) requests: Mutex<Vec<WorkflowStartRequest>>,
    pub(crate) lookups: Mutex<VecDeque<Result<WorkflowStartLookupResponse, String>>>,
    pub(crate) lookup_requests: Mutex<Vec<WorkflowStartLookupRequest>>,
}

#[async_trait]
impl WorkflowControlClient for ControlStub {
    async fn start(
        &self,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        self.requests.lock().await.push(request);
        self.results
            .lock()
            .await
            .pop_front()
            .expect("unexpected workflow start")
    }

    async fn lookup_start(
        &self,
        request: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, String> {
        self.lookup_requests.lock().await.push(request);
        self.lookups
            .lock()
            .await
            .pop_front()
            .unwrap_or(Ok(WorkflowStartLookupResponse::NotFound))
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
