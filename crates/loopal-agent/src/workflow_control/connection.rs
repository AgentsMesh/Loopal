use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_ipc::RpcError;
use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    MAX_WORKFLOW_WAIT_MS, WorkflowCancelRequest, WorkflowCancelResponse, WorkflowGetRequest,
    WorkflowGetResponse, WorkflowRequestId, WorkflowStartLookupRequest,
    WorkflowStartLookupResponse, WorkflowStartRequest, WorkflowStartResponse, WorkflowWaitRequest,
    WorkflowWaitResponse,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{WorkflowControlClient, WorkflowStartControlError};

const CONTROL_RPC_TIMEOUT: Duration = Duration::from_secs(30);
const WAIT_RPC_OVERHEAD: Duration = Duration::from_secs(5);

pub struct ConnectionWorkflowControlClient {
    connection: Arc<Connection<Listening>>,
}

impl ConnectionWorkflowControlClient {
    pub fn new(connection: Arc<Connection<Listening>>) -> Self {
        Self { connection }
    }

    async fn call<Request: Serialize, Response: DeserializeOwned>(
        &self,
        method: &str,
        request: Request,
        timeout: Duration,
    ) -> Result<Response, String> {
        let params = serde_json::to_value(request).map_err(|error| error.to_string())?;
        let response = tokio::time::timeout(timeout, self.connection.send_request(method, params))
            .await
            .map_err(|_| format!("{method} timed out"))?
            .map_err(|error| format!("{method} failed: {error}"))?;
        serde_json::from_value(response)
            .map_err(|error| format!("{method} returned an invalid response: {error}"))
    }
}

#[async_trait]
impl WorkflowControlClient for ConnectionWorkflowControlClient {
    async fn start(
        &self,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        let method = methods::HUB_WORKFLOW_START.name;
        let request_id = request.request_id.clone();
        let params = serde_json::to_value(request)
            .map_err(|error| WorkflowStartControlError::Rejected(error.to_string()))?;
        let response = tokio::time::timeout(
            CONTROL_RPC_TIMEOUT,
            self.connection.send_request(method, params),
        )
        .await
        .map_err(|_| indeterminate(request_id.clone(), format!("{method} timed out")))?
        .map_err(|error| classify_start_rpc_error(request_id.clone(), error))?;
        decode_start_response(request_id, response)
    }

    async fn get(&self, request: WorkflowGetRequest) -> Result<WorkflowGetResponse, String> {
        self.call(methods::HUB_WORKFLOW_GET.name, request, CONTROL_RPC_TIMEOUT)
            .await
    }

    async fn lookup_start(
        &self,
        request: WorkflowStartLookupRequest,
    ) -> Result<WorkflowStartLookupResponse, String> {
        self.call(
            methods::HUB_WORKFLOW_LOOKUP_START.name,
            request,
            CONTROL_RPC_TIMEOUT,
        )
        .await
    }

    async fn wait(&self, request: WorkflowWaitRequest) -> Result<WorkflowWaitResponse, String> {
        if request.timeout_ms > MAX_WORKFLOW_WAIT_MS {
            return Err(format!(
                "workflow wait exceeds the {MAX_WORKFLOW_WAIT_MS}ms limit"
            ));
        }
        let timeout = Duration::from_millis(request.timeout_ms).saturating_add(WAIT_RPC_OVERHEAD);
        self.call(methods::HUB_WORKFLOW_WAIT.name, request, timeout)
            .await
    }

    async fn cancel(
        &self,
        request: WorkflowCancelRequest,
    ) -> Result<WorkflowCancelResponse, String> {
        self.call(
            methods::HUB_WORKFLOW_CANCEL.name,
            request,
            CONTROL_RPC_TIMEOUT,
        )
        .await
    }
}

pub(super) fn classify_start_rpc_error(
    request_id: WorkflowRequestId,
    error: RpcError,
) -> WorkflowStartControlError {
    match error {
        RpcError::Remote { .. } => WorkflowStartControlError::Rejected(error.to_string()),
        RpcError::Transport(_) | RpcError::ChannelDropped => {
            indeterminate(request_id, error.to_string())
        }
    }
}

pub(super) fn decode_start_response(
    request_id: WorkflowRequestId,
    response: serde_json::Value,
) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
    serde_json::from_value(response).map_err(|error| {
        indeterminate(
            request_id,
            format!(
                "{} returned an invalid response: {error}",
                methods::HUB_WORKFLOW_START.name
            ),
        )
    })
}

fn indeterminate(request_id: WorkflowRequestId, message: String) -> WorkflowStartControlError {
    WorkflowStartControlError::Indeterminate {
        request_id,
        message,
    }
}
