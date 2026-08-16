#[tokio::test]
async fn live_capacity_rejects_overflow_without_evicting_existing_deliveries() {
    let session = session();
    let mut notifications = Vec::new();
    let mut requests = Vec::new();

    for terminal_revision in 1..=MAX_PENDING_WORKFLOW_TERMINALS as u64 {
        let next = notification(terminal_revision);
        let (request, receiver) = WorkflowTerminalRequest::tracked(next.clone());
        assert!(matches!(
            session.claim_workflow_terminal(&next, receiver).await,
            WorkflowTerminalClaim::New
        ));
        notifications.push(next);
        requests.push(request);
    }

    let overflow = notification(MAX_PENDING_WORKFLOW_TERMINALS as u64 + 1);
    let (overflow_request, receiver) = WorkflowTerminalRequest::tracked(overflow.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&overflow, receiver).await,
        WorkflowTerminalClaim::Full
    ));

    for existing in &notifications {
        let (duplicate_request, receiver) = WorkflowTerminalRequest::tracked(existing.clone());
        assert!(matches!(
            session.claim_workflow_terminal(existing, receiver).await,
            WorkflowTerminalClaim::Pending
        ));
        drop(duplicate_request);
    }

    drop(overflow_request);
    drop(requests);
}

#[tokio::test]
async fn late_retryable_delivery_lease_can_be_retried() {
    let session = session();
    let first = notification(1);
    let (request, receiver) = WorkflowTerminalRequest::tracked(first.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&first, receiver).await,
        WorkflowTerminalClaim::New
    ));
    request
        .acknowledge(loopal_protocol::WorkflowTerminalDisposition::Retryable {
            reason: "turn store unavailable".into(),
        })
        .await;
    drop(request);

    let (retry_request, retry_receiver) = WorkflowTerminalRequest::tracked(first.clone());
    assert!(matches!(
        session
            .claim_workflow_terminal(&first, retry_receiver)
            .await,
        WorkflowTerminalClaim::New
    ));
    drop(retry_request);
}

#[tokio::test]
async fn abandoned_unacknowledged_delivery_can_be_reclaimed() {
    let session = session();
    let terminal = notification(4);
    let (abandoned, receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&terminal, receiver).await,
        WorkflowTerminalClaim::New
    ));
    drop(abandoned);

    let (replacement, receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&terminal, receiver).await,
        WorkflowTerminalClaim::New
    ));
    drop(replacement);
}

#[tokio::test]
async fn capacity_pressure_evicts_completed_tombstones_before_live_deliveries() {
    let session = session();
    let completed = notification(10);
    let (completed_request, receiver) = WorkflowTerminalRequest::tracked(completed.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&completed, receiver).await,
        WorkflowTerminalClaim::New
    ));
    completed_request
        .acknowledge(loopal_protocol::WorkflowTerminalDisposition::Rejected {
            reason: "permanent".into(),
        })
        .await;

    let mut live_requests = Vec::new();
    for terminal_revision in 11..(10 + MAX_PENDING_WORKFLOW_TERMINALS as u64) {
        let live = notification(terminal_revision);
        let (request, receiver) = WorkflowTerminalRequest::tracked(live.clone());
        assert!(matches!(
            session.claim_workflow_terminal(&live, receiver).await,
            WorkflowTerminalClaim::New
        ));
        live_requests.push(request);
    }

    let overflow = notification(10 + MAX_PENDING_WORKFLOW_TERMINALS as u64);
    let (overflow_request, receiver) = WorkflowTerminalRequest::tracked(overflow.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&overflow, receiver).await,
        WorkflowTerminalClaim::New
    ));

    drop((completed_request, overflow_request, live_requests));
}
