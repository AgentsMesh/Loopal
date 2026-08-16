use async_trait::async_trait;
use loopal_protocol::{
    WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest, WorkflowGetResponse,
    WorkflowRequestId, WorkflowStartLookupRequest, WorkflowStartLookupResponse,
    WorkflowStartRequest, WorkflowStartResponse, WorkflowWaitRequest, WorkflowWaitResponse,
};

mod connection;
pub use connection::ConnectionWorkflowControlClient;

#[async_trait]
pub trait WorkflowControlClient: Send + Sync {
    async fn start(
        &self,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError>;

    /// A transport failure may happen after the Hub committed the start. Replay
    /// the exact request once so the Hub ledger can return the original result;
    /// a second ambiguous failure remains typed for the caller to handle.
    async fn start_with_confirmation(
        &self,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        let request_id = request.request_id.clone();
        match self.start(request.clone()).await {
            Err(WorkflowStartControlError::Indeterminate { message, .. }) => {
                match self.confirm_start(request).await {
                    Ok(response) => Ok(response),
                    Err(error) => Err(WorkflowStartControlError::Indeterminate {
                        request_id,
                        message: format!("{message}; confirmation failed: {error}"),
                    }),
                }
            }
            outcome => outcome,
        }
    }

    /// Confirm a request that has already crossed an ambiguous transport
    /// boundary. Only a successful Hub replay resolves it; later errors cannot
    /// prove that the original request was rejected before commit.
    async fn confirm_start(
        &self,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        let request_id = request.request_id.clone();
        match self.start(request).await {
            Ok(response) => Ok(response),
            Err(error) => Err(WorkflowStartControlError::Indeterminate {
                request_id,
                message: format!("confirmation failed: {error}"),
            }),
        }
    }

    async fn lookup_start(
        &self,
        _request: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, String> {
        Err("workflow start lookup is unavailable".into())
    }

    async fn get(&self, request: WorkflowGetRequest) -> Result<WorkflowGetResponse, String>;
    async fn wait(&self, request: WorkflowWaitRequest) -> Result<WorkflowWaitResponse, String>;
    async fn cancel(
        &self,
        request: WorkflowCancelRequest,
    ) -> Result<WorkflowCancelResponse, String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowStartControlError {
    Rejected(String),
    Indeterminate {
        request_id: WorkflowRequestId,
        message: String,
    },
}

impl std::fmt::Display for WorkflowStartControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(message) => write!(formatter, "workflow start rejected: {message}"),
            Self::Indeterminate {
                request_id,
                message,
            } => write!(
                formatter,
                "workflow start outcome for request_id {request_id} is indeterminate: {message}"
            ),
        }
    }
}

impl std::error::Error for WorkflowStartControlError {}

#[cfg(test)]
#[path = "workflow_control_tests.rs"]
mod tests;
