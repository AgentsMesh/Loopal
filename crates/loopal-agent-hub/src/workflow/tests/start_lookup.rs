use loopal_protocol::{
    WorkflowRequestId, WorkflowRequestLedger, WorkflowStartLookupRequest,
    WorkflowStartLookupResponse,
};

use super::super::{WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::journal_support::TestJournal;
use super::support::{
    coordinator_with_journal, coordinator_with_storage, get_request, owner, request,
};

#[tokio::test]
async fn lookup_recovers_durable_start_and_operation_collision() {
    let (first, first_task, _, _, journal) = coordinator_with_journal(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        ["wrun_lookup".into()],
    );
    let workflow_owner = owner("session-lookup", "root");
    let start = first
        .start(workflow_owner.clone(), request("wreq_existing"))
        .await
        .unwrap();
    let recovered_run = first
        .get(
            workflow_owner.clone(),
            get_request("wreq_other_operation", start.summary.id.clone()),
        )
        .await
        .unwrap()
        .run
        .expect("the get record must retain the started run");
    let start_record = journal
        .starts()
        .into_iter()
        .next()
        .expect("start must be durably appended");
    let mut ledger = WorkflowRequestLedger::default();
    ledger.record(start_record.request).unwrap();
    for (_, _, record) in journal.requests() {
        ledger.record(record).unwrap();
    }
    journal.push_recovery(Ok(super::super::recovery::RecoveredOwner {
        runs: vec![recovered_run],
        requests: ledger,
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    drop(first);
    first_task.await.unwrap();

    let (recovered, recovered_task, _, _) =
        coordinator_with_storage(WorkflowCoordinatorMode::Preview, [], [], journal);
    assert_eq!(
        recovered
            .lookup_start(workflow_owner.clone(), lookup_request("wreq_existing"),)
            .await
            .unwrap(),
        WorkflowStartLookupResponse::Found { response: start }
    );
    assert_eq!(
        recovered
            .lookup_start(
                workflow_owner.clone(),
                lookup_request("wreq_other_operation"),
            )
            .await
            .unwrap(),
        WorkflowStartLookupResponse::Conflict
    );
    assert_eq!(
        recovered
            .lookup_start(workflow_owner, lookup_request("wreq_absent"))
            .await
            .unwrap(),
        WorkflowStartLookupResponse::NotFound
    );
    drop(recovered);
    recovered_task.await.unwrap();
}

#[tokio::test]
async fn lookup_is_owner_scoped_and_rejects_invalid_ids() {
    let (handle, task, _, _) = coordinator_with_storage(
        WorkflowCoordinatorMode::Preview,
        [],
        [],
        std::sync::Arc::new(TestJournal::new()),
    );
    assert_eq!(
        handle
            .lookup_start(
                owner("other-session", "root"),
                lookup_request("wreq_absent")
            )
            .await
            .unwrap(),
        WorkflowStartLookupResponse::NotFound
    );
    assert!(matches!(
        handle
            .lookup_start(owner("other-session", "root"), lookup_request(""))
            .await,
        Err(WorkflowCoordinatorError::Request(
            loopal_protocol::WorkflowRequestError::InvalidRequestId
        ))
    ));
    drop(handle);
    task.await.unwrap();
}

fn lookup_request(request_id: &str) -> WorkflowStartLookupRequest {
    WorkflowStartLookupRequest {
        request_id: WorkflowRequestId::new(request_id),
    }
}
