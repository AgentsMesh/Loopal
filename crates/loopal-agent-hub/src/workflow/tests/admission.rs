use std::sync::Arc;

use loopal_protocol::{
    WorkflowOutputContract, WorkflowRunId, WorkflowRunState, WorkflowValidationError,
};
use loopal_workflow_schema::WorkflowSchemaError;

use super::super::{WorkflowCoordinator, WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::support::{coordinator, get_request, owner, request};

#[tokio::test]
async fn disabled_rejects_before_clock_id_or_state_changes() {
    let (handle, task, clock, ids) = coordinator(WorkflowCoordinatorMode::Disabled, [], []);
    assert_eq!(
        handle
            .start(owner("session", "root"), request("wreq_start"))
            .await,
        Err(WorkflowCoordinatorError::Disabled)
    );
    assert_eq!(clock.calls(), 0);
    assert_eq!(ids.calls(), 0);
    assert_eq!(
        handle
            .get(
                owner("session", "root"),
                get_request("wreq_get_disabled", WorkflowRunId::new("wrun_absent")),
            )
            .await,
        Err(WorkflowCoordinatorError::Disabled)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn production_constructor_is_disabled() {
    let (handle, task) = WorkflowCoordinator::spawn_disabled();
    assert_eq!(
        handle
            .start(owner("session", "root"), request("wreq_disabled"))
            .await,
        Err(WorkflowCoordinatorError::Disabled)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn production_storage_constructor_is_disabled() {
    let temp = tempfile::tempdir().unwrap();
    let sessions = Arc::new(loopal_storage::SessionStore::with_base_dir(
        temp.path().to_path_buf(),
    ));
    let (handle, task) = WorkflowCoordinator::spawn_disabled_with_storage(sessions);
    assert_eq!(
        handle
            .start(owner("session", "root"), request("wreq_disabled_storage"))
            .await,
        Err(WorkflowCoordinatorError::Disabled)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn preview_stores_only_a_validated_snapshot() {
    let (handle, task, clock, ids) = coordinator(
        WorkflowCoordinatorMode::Preview,
        [200, 100],
        [WorkflowRunId::new("wrun_preview")],
    );
    let owner = owner("session", "root");
    let response = handle
        .start(owner.clone(), request("wreq_start"))
        .await
        .unwrap();
    assert_eq!(response.summary.state, WorkflowRunState::Validated);
    assert_eq!(response.summary.revision, 1);
    assert_eq!(response.summary.created_at_unix_ms, 200);
    assert_eq!(response.summary.updated_at_unix_ms, 200);
    assert_eq!(response.summary.counts.pending, 2);
    assert_eq!(response.summary.counts.ready, 0);

    let run = handle
        .get(
            owner,
            get_request("wreq_get_preview", WorkflowRunId::new("wrun_preview")),
        )
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(run.state, WorkflowRunState::Validated);
    assert!(
        run.nodes
            .iter()
            .all(|node| node.state == loopal_protocol::WorkflowNodeState::Pending)
    );
    assert!(run.attempts.is_empty());
    assert_eq!(clock.calls(), 2);
    assert_eq!(ids.calls(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn invalid_owner_spec_and_schema_do_not_consume_ids() {
    let (handle, task, _, ids) = coordinator(
        WorkflowCoordinatorMode::Preview,
        [],
        [WorkflowRunId::new("wrun_unused")],
    );
    assert_eq!(
        handle
            .start(owner("../session", "root"), request("wreq_owner"))
            .await,
        Err(WorkflowCoordinatorError::InvalidOwner)
    );

    let mut invalid_spec = request("wreq_spec");
    invalid_spec.spec.run_goal.clear();
    assert_eq!(
        handle.start(owner("session", "root"), invalid_spec).await,
        Err(WorkflowCoordinatorError::Validation(
            WorkflowValidationError::EmptyGoal
        ))
    );

    let mut invalid_schema = request("wreq_schema");
    invalid_schema.spec.output_contract = WorkflowOutputContract::Json {
        max_bytes: 1_024,
        schema: serde_json::json!({"type": "not-a-json-type"}),
    };
    assert_eq!(
        handle.start(owner("session", "root"), invalid_schema).await,
        Err(WorkflowCoordinatorError::Schema(
            WorkflowSchemaError::InvalidSchema
        ))
    );
    assert_eq!(ids.calls(), 0);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn invalid_generated_id_does_not_commit_request() {
    let (handle, task, _, ids) = coordinator(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [WorkflowRunId::new(""), WorkflowRunId::new("wrun_retry")],
    );
    let owner = owner("session", "root");
    let request = request("wreq_retry");
    assert_eq!(
        handle.start(owner.clone(), request.clone()).await,
        Err(WorkflowCoordinatorError::InvalidGeneratedRunId(
            WorkflowRunId::new("")
        ))
    );
    let response = handle.start(owner, request).await.unwrap();
    assert_eq!(response.summary.id, WorkflowRunId::new("wrun_retry"));
    assert_eq!(ids.calls(), 2);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn run_id_collision_preserves_the_existing_run() {
    let duplicate = WorkflowRunId::new("wrun_same");
    let (handle, task, _, ids) = coordinator(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [duplicate.clone(), duplicate.clone()],
    );
    let owner = owner("session", "root");
    let first = handle
        .start(owner.clone(), request("wreq_first"))
        .await
        .unwrap();
    assert_eq!(
        handle.start(owner.clone(), request("wreq_second")).await,
        Err(WorkflowCoordinatorError::RunIdCollision(duplicate.clone()))
    );
    let stored = handle
        .get(owner, get_request("wreq_get_same", duplicate))
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(stored.spec.run_goal, first.summary.run_goal);
    assert_eq!(ids.calls(), 2);
    drop(handle);
    task.await.unwrap();
}
