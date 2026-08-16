use std::time::Duration;

use loopal_protocol::WorkflowTerminalDisposition;
use loopal_runtime::agent_input::AgentInput;

use super::core_tests::{notification, peers, request_parts, session};
use super::forward_with_timeout;

#[tokio::test]
async fn retryable_runtime_result_releases_lease_for_retry() {
    let ((server, mut incoming), (client, _)) = peers();
    let (session, mut input_rx) = session("session-retry");
    let terminal = notification("session-retry");

    let (first, id, params) = request_parts(client.clone(), &mut incoming, &terminal).await;
    let first_forward = forward_with_timeout(id, params, &session, &server, Duration::from_secs(1));
    let first_runtime = async {
        let AgentInput::WorkflowTerminal(request) = input_rx.recv().await.unwrap() else {
            panic!("expected first workflow terminal")
        };
        request
            .acknowledge(WorkflowTerminalDisposition::Retryable {
                reason: "turn store is temporarily unavailable".into(),
            })
            .await;
    };
    tokio::join!(first_forward, first_runtime);
    assert_eq!(first.await.unwrap().unwrap()["status"], "retryable");

    let (retry, id, params) = request_parts(client, &mut incoming, &terminal).await;
    let retry_forward = forward_with_timeout(id, params, &session, &server, Duration::from_secs(1));
    let retry_runtime = async {
        let AgentInput::WorkflowTerminal(request) = input_rx.recv().await.unwrap() else {
            panic!("expected retried workflow terminal")
        };
        request
            .acknowledge(WorkflowTerminalDisposition::Applied)
            .await;
    };
    tokio::join!(retry_forward, retry_runtime);
    assert_eq!(retry.await.unwrap().unwrap()["status"], "applied");
}
