use std::time::Duration;

use loopal_protocol::WorkflowTerminalDisposition;
use loopal_runtime::agent_input::AgentInput;

use super::core_tests::{notification, peers, request_parts, session};
use super::{AckWait, forward_with_timeout, wait_for_acknowledgement};

#[tokio::test]
async fn dropped_application_request_is_rejected_and_releases_delivery() {
    let ((server, mut incoming), (client, _)) = peers();
    let (session, mut input_rx) = session("session-closed-ack");
    let terminal = notification("session-closed-ack");
    let (response, id, params) = request_parts(client.clone(), &mut incoming, &terminal).await;

    tokio::join!(
        forward_with_timeout(id, params, &session, &server, Duration::from_secs(1)),
        async {
            let AgentInput::WorkflowTerminal(request) = input_rx.recv().await.unwrap() else {
                panic!("expected workflow terminal")
            };
            drop(request);
        }
    );
    assert_eq!(
        response.await.unwrap().unwrap_err().remote_code(),
        Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
    );

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

#[tokio::test]
async fn wait_handles_prefilled_and_empty_changes() {
    let (_sender, mut prefilled) =
        tokio::sync::watch::channel(Some(WorkflowTerminalDisposition::AlreadyApplied));
    assert_eq!(
        wait_for_acknowledgement(&mut prefilled, Duration::ZERO).await,
        AckWait::Received(WorkflowTerminalDisposition::AlreadyApplied)
    );

    let (sender, mut changed_to_none) = tokio::sync::watch::channel(None);
    let _ = sender.send_replace(None);
    assert_eq!(
        wait_for_acknowledgement(&mut changed_to_none, Duration::from_secs(1)).await,
        AckWait::Closed
    );
}
