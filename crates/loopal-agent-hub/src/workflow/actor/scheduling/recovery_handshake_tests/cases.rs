use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptId, WorkflowAttemptState, WorkflowNodeId, WorkflowRunId,
};

use super::{expect_error, fixture};
use crate::workflow::scheduler::ActiveAttemptPhase;
use crate::workflow::{WorkflowCoordinatorError, WorkflowOwner};

#[tokio::test]
async fn every_inconsistent_active_identity_fails_closed() {
    let (mut coordinator, owner, request) = fixture(false);
    let active = coordinator
        .active
        .get_mut(&request.causation.attempt_id)
        .unwrap();
    active.owner = WorkflowOwner::new("other-session", QualifiedAddress::local("root"));
    expect_error(
        &mut coordinator,
        &owner,
        &request,
        WorkflowCoordinatorError::StaleExecutionLease,
    )
    .await;
    let active = coordinator
        .active
        .get_mut(&request.causation.attempt_id)
        .unwrap();
    active.owner = owner.clone();
    active.key.run_id = WorkflowRunId::new("wrun_other");
    expect_error(
        &mut coordinator,
        &owner,
        &request,
        WorkflowCoordinatorError::StaleExecutionLease,
    )
    .await;
    let active = coordinator
        .active
        .get_mut(&request.causation.attempt_id)
        .unwrap();
    active.key.run_id = request.causation.run_id.clone();
    active.key.node_id = WorkflowNodeId::new("other-node");
    expect_error(
        &mut coordinator,
        &owner,
        &request,
        WorkflowCoordinatorError::StaleExecutionLease,
    )
    .await;
    let active = coordinator
        .active
        .get_mut(&request.causation.attempt_id)
        .unwrap();
    active.key.node_id = request.causation.node_id.clone();
    active.key.attempt_id = WorkflowAttemptId::new("watt_other");
    expect_error(
        &mut coordinator,
        &owner,
        &request,
        WorkflowCoordinatorError::StaleExecutionLease,
    )
    .await;
    let active = coordinator
        .active
        .get_mut(&request.causation.attempt_id)
        .unwrap();
    active.key.attempt_id = request.causation.attempt_id.clone();
    active.phase = ActiveAttemptPhase::Interrupting;
    expect_error(
        &mut coordinator,
        &owner,
        &request,
        WorkflowCoordinatorError::StaleExecutionLease,
    )
    .await;
}

#[tokio::test]
async fn active_phase_cannot_override_an_invalid_attempt_state() {
    let (mut coordinator, owner, request) = fixture(true);
    assert_eq!(
        coordinator
            .state
            .owned_snapshot(&owner, &request.causation.run_id)
            .unwrap()
            .attempts[0]
            .state,
        WorkflowAttemptState::Cancelling
    );
    expect_error(
        &mut coordinator,
        &owner,
        &request,
        WorkflowCoordinatorError::InvalidExecutionLease,
    )
    .await;
}
