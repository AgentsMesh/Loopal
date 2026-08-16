use loopal_protocol::{WorkflowRequestError, WorkflowRunId};

use super::super::{WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::support::{coordinator_with_journal, get_request, owner, request};

#[tokio::test]
async fn owned_get_is_journaled_once_and_replayed() {
    let run_id = WorkflowRunId::new("wrun_owned");
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [10, 11], [run_id.clone()]);
    let owner = owner("session", "root");
    handle
        .start(owner.clone(), request("wreq_start"))
        .await
        .unwrap();

    let request = get_request("wreq_get", run_id.clone());
    let first = handle.get(owner.clone(), request.clone()).await.unwrap();
    let replay = handle.get(owner.clone(), request).await.unwrap();

    assert_eq!(replay, first);
    let requests = journal.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, owner);
    assert_eq!(requests[0].1, run_id);
    assert_eq!(requests[0].2.operation, "get");
    assert_eq!(requests[0].2.response, serde_json::to_value(first).unwrap());
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn get_request_ids_bind_operation_and_run() {
    let run_id = WorkflowRunId::new("wrun_bound");
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [10, 11], [run_id.clone()]);
    let owner = owner("session", "root");
    handle
        .start(owner.clone(), request("wreq_start"))
        .await
        .unwrap();

    assert_eq!(
        handle
            .get(
                owner.clone(),
                get_request("", WorkflowRunId::new("wrun_missing")),
            )
            .await,
        Err(WorkflowCoordinatorError::Request(
            WorkflowRequestError::InvalidRequestId,
        ))
    );
    assert_eq!(
        handle
            .get(owner.clone(), get_request("wreq_start", run_id.clone()))
            .await,
        Err(WorkflowCoordinatorError::Request(
            WorkflowRequestError::PayloadMismatch {
                request_id: "wreq_start".into(),
            },
        ))
    );

    handle
        .get(owner.clone(), get_request("wreq_get", run_id))
        .await
        .unwrap();
    assert_eq!(
        handle
            .get(
                owner,
                get_request("wreq_get", WorkflowRunId::new("wrun_other")),
            )
            .await,
        Err(WorkflowCoordinatorError::Request(
            WorkflowRequestError::PayloadMismatch {
                request_id: "wreq_get".into(),
            },
        ))
    );
    assert_eq!(journal.requests().len(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn absent_or_cross_owner_get_does_not_touch_a_run_journal() {
    let run_id = WorkflowRunId::new("wrun_private");
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [10, 11], [run_id.clone()]);
    let first_owner = owner("session-a", "root");
    handle
        .start(first_owner.clone(), request("wreq_start"))
        .await
        .unwrap();

    let cross_owner = get_request("wreq_cross", run_id);
    let first = handle
        .get(owner("session-b", "root"), cross_owner.clone())
        .await
        .unwrap();
    let replay = handle
        .get(owner("session-b", "root"), cross_owner)
        .await
        .unwrap();
    let absent = handle
        .get(
            first_owner,
            get_request("wreq_absent", WorkflowRunId::new("wrun_absent")),
        )
        .await
        .unwrap();

    assert_eq!(replay, first);
    assert!(first.run.is_none());
    assert!(absent.run.is_none());
    assert!(journal.requests().is_empty());
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn get_append_failure_poisons_owner_without_committing_request() {
    let run_id = WorkflowRunId::new("wrun_poisoned");
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [10, 11], [run_id.clone()]);
    let owner = owner("session", "root");
    handle
        .start(owner.clone(), request("wreq_start"))
        .await
        .unwrap();
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    let request = get_request("wreq_get", run_id);

    assert_eq!(
        handle.get(owner.clone(), request.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert!(journal.requests().is_empty());
    assert_eq!(
        handle.get(owner, request).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn get_append_task_failure_also_poisons_owner() {
    let run_id = WorkflowRunId::new("wrun_ambiguous_get");
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [10, 11], [run_id.clone()]);
    let owner = owner("session", "root");
    handle
        .start(owner.clone(), request("wreq_start"))
        .await
        .unwrap();
    journal.push_append_panic();
    let request = get_request("wreq_get", run_id);

    assert_eq!(
        handle.get(owner.clone(), request.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        handle.get(owner, request).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    drop(handle);
    task.await.unwrap();
}
