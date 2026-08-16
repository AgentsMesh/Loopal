use super::connection::{classify_start_rpc_error, decode_start_response};
use super::*;
use loopal_ipc::RpcError;

struct DefaultLookupClient;

#[async_trait::async_trait]
impl WorkflowControlClient for DefaultLookupClient {
    async fn start(
        &self,
        _request: WorkflowStartRequest,
    ) -> Result<WorkflowStartResponse, WorkflowStartControlError> {
        unreachable!("default lookup must not start a workflow")
    }

    async fn get(&self, _request: WorkflowGetRequest) -> Result<WorkflowGetResponse, String> {
        unreachable!("default lookup must not get a workflow")
    }

    async fn wait(&self, _request: WorkflowWaitRequest) -> Result<WorkflowWaitResponse, String> {
        unreachable!("default lookup must not wait for a workflow")
    }

    async fn cancel(
        &self,
        _request: WorkflowCancelRequest,
    ) -> Result<WorkflowCancelResponse, String> {
        unreachable!("default lookup must not cancel a workflow")
    }
}

#[tokio::test]
async fn default_start_lookup_reports_unavailable() {
    let error = DefaultLookupClient
        .lookup_start(WorkflowStartLookupRequest {
            request_id: "wreq_default_lookup".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(error, "workflow start lookup is unavailable");
}

#[test]
fn start_error_classification_preserves_commit_uncertainty() {
    let rejected = RpcError::Remote {
        code: -32600,
        message: "request rejected".into(),
        data: None,
    };
    assert!(matches!(
        classify_start_rpc_error("wreq_rejected".into(), rejected),
        WorkflowStartControlError::Rejected(_)
    ));
    for error in [
        RpcError::Transport("connection lost".into()),
        RpcError::ChannelDropped,
    ] {
        assert!(matches!(
            classify_start_rpc_error("wreq_ambiguous".into(), error),
            WorkflowStartControlError::Indeterminate { request_id, .. }
                if request_id.as_str() == "wreq_ambiguous"
        ));
    }
    assert!(matches!(
        decode_start_response(
            "wreq_decode".into(),
            serde_json::json!({"unexpected": true})
        ),
        Err(WorkflowStartControlError::Indeterminate { request_id, .. })
            if request_id.as_str() == "wreq_decode"
    ));
}

#[test]
fn start_error_text_preserves_the_request_id() {
    let error = WorkflowStartControlError::Indeterminate {
        request_id: "wreq_lost".into(),
        message: "response lost".into(),
    };
    assert!(error.to_string().contains("wreq_lost"));
    assert!(
        WorkflowStartControlError::Rejected("invalid graph".into())
            .to_string()
            .contains("rejected")
    );
}

#[path = "workflow_connection_tests.rs"]
mod connection_tests;
