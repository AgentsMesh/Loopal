#[tokio::test]
async fn discard_with_the_wrong_digest_preserves_the_live_lease() {
    let session = session();
    let terminal = notification(5);
    let (request, receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
    assert!(matches!(
        session
            .claim_workflow_terminal(&terminal, receiver.clone())
            .await,
        WorkflowTerminalClaim::New
    ));

    session
        .discard_workflow_terminal(&terminal.delivery_id, "wrong-digest", &receiver)
        .await;

    let (duplicate, duplicate_receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
    assert!(matches!(
        session
            .claim_workflow_terminal(&terminal, duplicate_receiver)
            .await,
        WorkflowTerminalClaim::Pending
    ));
    drop((request, duplicate));
}

#[tokio::test]
async fn permanent_rejection_is_cached_and_payload_conflict_stays_rejected() {
    let session = session();
    let first = notification(2);
    let (request, receiver) = WorkflowTerminalRequest::tracked(first.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&first, receiver).await,
        WorkflowTerminalClaim::New
    ));
    request
        .acknowledge(loopal_protocol::WorkflowTerminalDisposition::Rejected {
            reason: "persisted payload conflict".into(),
        })
        .await;
    drop(request);

    let (duplicate, receiver) = WorkflowTerminalRequest::tracked(first.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&first, receiver).await,
        WorkflowTerminalClaim::Completed(
            loopal_protocol::WorkflowTerminalDisposition::Rejected { .. }
        )
    ));
    drop(duplicate);

    let mut conflict = first;
    conflict.content.push('!');
    let (conflicting, receiver) = WorkflowTerminalRequest::tracked(conflict.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&conflict, receiver).await,
        WorkflowTerminalClaim::Conflict
    ));
    drop(conflicting);
}

#[tokio::test]
async fn stale_discard_cannot_remove_a_replacement_lease() {
    let session = session();
    let terminal = notification(3);
    let digest = terminal.payload_digest();
    let (first, first_receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
    assert!(matches!(
        session
            .claim_workflow_terminal(&terminal, first_receiver.clone())
            .await,
        WorkflowTerminalClaim::New
    ));
    first
        .acknowledge(loopal_protocol::WorkflowTerminalDisposition::Retryable {
            reason: "retry".into(),
        })
        .await;

    let (replacement, replacement_receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
    assert!(matches!(
        session
            .claim_workflow_terminal(&terminal, replacement_receiver)
            .await,
        WorkflowTerminalClaim::New
    ));
    session
        .discard_workflow_terminal(&terminal.delivery_id, &digest, &first_receiver)
        .await;

    let (duplicate, receiver) = WorkflowTerminalRequest::tracked(terminal.clone());
    assert!(matches!(
        session.claim_workflow_terminal(&terminal, receiver).await,
        WorkflowTerminalClaim::Pending
    ));
    drop((first, replacement, duplicate));
}
