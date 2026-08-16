use loopal_protocol::{WorkflowRequestError, WorkflowRunId};

use super::super::{WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::support::{coordinator, get_request, owner, request};

#[tokio::test]
async fn identical_start_request_replays_without_new_admission() {
    let (handle, task, clock, ids) = coordinator(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [WorkflowRunId::new("wrun_once")],
    );
    let owner = owner("session", "root");
    let request = request("wreq_same");
    let first = handle.start(owner.clone(), request.clone()).await.unwrap();
    let replay = handle.start(owner, request).await.unwrap();
    assert_eq!(replay, first);
    assert_eq!(clock.calls(), 2);
    assert_eq!(ids.calls(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn reused_request_id_with_different_payload_fails_closed() {
    let (handle, task, _, ids) = coordinator(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [WorkflowRunId::new("wrun_first")],
    );
    let owner = owner("session", "root");
    handle
        .start(owner.clone(), request("wreq_same"))
        .await
        .unwrap();
    let mut changed = request("wreq_same");
    changed.spec.run_goal = "different goal".into();
    assert_eq!(
        handle.start(owner, changed).await,
        Err(WorkflowCoordinatorError::Request(
            WorkflowRequestError::PayloadMismatch {
                request_id: "wreq_same".into(),
            }
        ))
    );
    assert_eq!(ids.calls(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn request_and_run_access_are_scoped_to_owner() {
    let (handle, task, _, ids) = coordinator(
        WorkflowCoordinatorMode::Preview,
        [10, 11, 20, 21],
        [
            WorkflowRunId::new("wrun_first"),
            WorkflowRunId::new("wrun_second"),
        ],
    );
    let first_owner = owner("session-a", "root");
    let second_owner = owner("session-b", "root");
    let first = handle
        .start(first_owner.clone(), request("wreq_shared"))
        .await
        .unwrap();
    let second = handle
        .start(second_owner.clone(), request("wreq_shared"))
        .await
        .unwrap();
    assert_ne!(first.summary.id, second.summary.id);
    assert!(
        handle
            .get(
                second_owner,
                get_request("wreq_get_absent", first.summary.id.clone()),
            )
            .await
            .unwrap()
            .run
            .is_none()
    );
    assert!(
        handle
            .get(first_owner, get_request("wreq_get_owned", first.summary.id),)
            .await
            .unwrap()
            .run
            .is_some()
    );
    assert_eq!(ids.calls(), 2);
    drop(handle);
    task.await.unwrap();
}
