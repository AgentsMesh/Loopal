use loopal_protocol::{WorkflowRequestId, WorkflowRunId, WorkflowWaitRequest, WorkflowWaitStatus};

use super::support::{coordinator, coordinator_with_journal, owner, request};
use crate::workflow::{WorkflowCoordinatorError, WorkflowCoordinatorMode};

fn wait_request(
    request_id: &str,
    run_id: impl Into<String>,
    after_revision: u64,
) -> WorkflowWaitRequest {
    WorkflowWaitRequest {
        request_id: WorkflowRequestId::new(request_id),
        run_id: WorkflowRunId::new(run_id),
        after_revision,
        timeout_ms: 0,
    }
}

#[tokio::test]
async fn wait_subscription_enforces_admission_and_tracks_existing_runs() {
    let disabled_request = wait_request("wreq_wait_disabled", "wrun_wait_disabled", 0);
    let (disabled, disabled_task, _, _) = coordinator(WorkflowCoordinatorMode::Disabled, [], []);
    assert_eq!(
        disabled
            .wait(owner("session-wait-disabled", "root"), disabled_request)
            .await,
        Err(WorkflowCoordinatorError::Disabled)
    );
    disabled.shutdown().await.unwrap();
    disabled_task.await.unwrap();

    let run_id = WorkflowRunId::new("wrun_wait_subscription");
    let (handle, task, _, _, _) = coordinator_with_journal(
        WorkflowCoordinatorMode::Preview,
        [100, 101],
        [run_id.clone()],
    );
    let valid_owner = owner("session-wait-subscription", "root");
    assert_eq!(
        handle
            .wait(
                owner("bad/session", "root"),
                wait_request("wreq_wait_invalid_owner", run_id.as_str(), 0),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidOwner)
    );
    assert_eq!(
        handle
            .wait(
                valid_owner.clone(),
                wait_request("wreq_wait_invalid_run", "", 0),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidRunId)
    );

    let missing = handle
        .wait(
            valid_owner.clone(),
            wait_request("wreq_wait_missing", "wrun_wait_missing", 0),
        )
        .await
        .unwrap();
    assert_eq!(missing.status, WorkflowWaitStatus::NotFound);

    handle
        .start(valid_owner.clone(), request("wreq_wait_start"))
        .await
        .unwrap();
    for request_id in ["wreq_wait_existing_first", "wreq_wait_existing_second"] {
        let response = handle
            .wait(
                valid_owner.clone(),
                wait_request(request_id, run_id.as_str(), 0),
            )
            .await
            .unwrap();
        assert_eq!(response.status, WorkflowWaitStatus::Changed);
        assert_eq!(response.run.unwrap().id, run_id);
    }

    handle.shutdown().await.unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn wait_subscription_rejects_a_poisoned_owner() {
    let run_id = WorkflowRunId::new("wrun_wait_poisoned");
    let (handle, task, _, _, journal) = coordinator_with_journal(
        WorkflowCoordinatorMode::Preview,
        [200, 201],
        [run_id.clone()],
    );
    let owner = owner("session-wait-poisoned", "root");
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    assert_eq!(
        handle
            .start(owner.clone(), request("wreq_wait_poison_start"))
            .await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        handle
            .wait(
                owner,
                wait_request("wreq_wait_poisoned", run_id.as_str(), 0),
            )
            .await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    handle.shutdown().await.unwrap();
    task.await.unwrap();
}
