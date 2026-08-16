use std::time::Duration;

use loopal_protocol::WorkflowTerminalDisposition;
use loopal_runtime::agent_input::WorkflowTerminalRequest;

use super::core_tests::{notification, peers, request_parts, session};
use super::{WorkflowTerminalClaim, forward_with_timeout};

#[tokio::test]
async fn live_pending_capacity_returns_retryable_without_enqueueing_overflow() {
    let ((server, mut incoming), (client, _)) = peers();
    let (session, mut input_rx) = session("session-full");
    let mut leases = Vec::new();
    let mut overflow = None;

    for revision in 1..=1024 {
        let mut terminal = notification("session-full");
        terminal.delivery_id.terminal_revision = revision;
        let (request, receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
        match session.claim_workflow_terminal(&terminal, receiver).await {
            WorkflowTerminalClaim::New => leases.push(request),
            WorkflowTerminalClaim::Full => {
                overflow = Some(terminal);
                break;
            }
            _ => panic!("unique pending delivery must be new until capacity is full"),
        }
    }

    let overflow = overflow.expect("pending workflow terminal capacity must be bounded");
    let (response, id, params) = request_parts(client, &mut incoming, &overflow).await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    let disposition =
        serde_json::from_value::<WorkflowTerminalDisposition>(response.await.unwrap().unwrap())
            .unwrap();
    assert!(matches!(
        disposition,
        WorkflowTerminalDisposition::Retryable { .. }
    ));
    assert!(input_rx.try_recv().is_err());
    drop(leases);
}
