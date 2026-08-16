use std::sync::Arc;

use async_trait::async_trait;
use loopal_agent::workflow_control::{WorkflowControlClient, WorkflowStartControlError};
use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowStartLookupRequest, WorkflowStartLookupResponse, WorkflowStartRequest,
    WorkflowStartResponse, WorkflowWaitRequest, WorkflowWaitResponse,
};

pub(crate) struct TrackingWorkflowControlClient {
    inner: Arc<dyn WorkflowControlClient>,
    leases: Arc<loopal_runtime::WorkflowLeaseTracker>,
}

impl TrackingWorkflowControlClient {
    pub(crate) fn new(
        inner: Arc<dyn WorkflowControlClient>,
        leases: Arc<loopal_runtime::WorkflowLeaseTracker>,
    ) -> Self {
        Self { inner, leases }
    }
}

#[async_trait]
impl WorkflowControlClient for TrackingWorkflowControlClient {
    async fn start(
        &self,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        let response = self.inner.start(request).await?;
        self.leases.track(response.summary.id.clone());
        Ok(response)
    }

    async fn get(&self, request: WorkflowGetRequest) -> Result<WorkflowGetResponse, String> {
        self.inner.get(request).await
    }

    async fn lookup_start(
        &self,
        request: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, String> {
        let response = self.inner.lookup_start(request).await?;
        if let WorkflowStartLookupResponse::Found { response: start } = &response {
            self.leases.track(start.summary.id.clone());
        }
        Ok(response)
    }

    async fn wait(&self, request: WorkflowWaitRequest) -> Result<WorkflowWaitResponse, String> {
        self.inner.wait(request).await
    }

    async fn cancel(
        &self,
        request: WorkflowCancelRequest,
    ) -> Result<WorkflowCancelResponse, String> {
        self.inner.cancel(request).await
    }
}

#[cfg(test)]
#[path = "workflow_control_tracking_tests.rs"]
mod tests;
