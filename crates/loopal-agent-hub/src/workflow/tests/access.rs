use std::sync::Arc;

use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptId, WorkflowEventPayload, WorkflowNodeId, WorkflowReduceError,
    WorkflowRequestError, WorkflowRunId, WorkflowRunSnapshot, WorkflowValidationError,
    WorkflowWorkerProfileRef,
};
use loopal_workflow_schema::WorkflowSchemaError;
use tokio::sync::mpsc;

use super::super::command::WorkflowCommand;
use super::super::transition::apply_payload;
use super::super::{
    SystemWorkflowClock, SystemWorkflowIdSource, WorkflowClock, WorkflowCoordinator,
    WorkflowCoordinatorError, WorkflowCoordinatorHandle, WorkflowCoordinatorMode, WorkflowIdSource,
    WorkflowOwner,
};
use super::journal_support::TestJournal;
use super::support::{TestClock, TestIds, coordinator, get_request, owner, request};

#[test]
fn owner_validation_rejects_remote_and_unsafe_components() {
    assert!(owner("session", "root").is_valid());
    for session in ["", ".", "..", "a/b", "a\\b"] {
        assert!(!owner(session, "root").is_valid());
    }
    assert!(!owner(&"x".repeat(129), "root").is_valid());
    assert!(!owner("session", "").is_valid());
    assert!(!WorkflowOwner::new("session", QualifiedAddress::remote(["hub"], "root")).is_valid());
}

#[test]
fn production_clock_and_id_sources_return_valid_values() {
    assert!(SystemWorkflowClock.now_unix_ms() > 0);
    assert!(SystemWorkflowIdSource.next_run_id().is_valid());
    assert!(SystemWorkflowIdSource.next_attempt_id().is_valid());
    assert_eq!(
        SystemWorkflowIdSource
            .next_attempt_capability()
            .expose()
            .len(),
        64
    );
}

#[test]
fn coordinator_errors_are_sanitized_and_convert_from_dependencies() {
    let errors = [
        WorkflowCoordinatorError::Disabled,
        WorkflowCoordinatorError::Unavailable,
        WorkflowCoordinatorError::InvalidOwner,
        WorkflowCoordinatorError::OwnerPoisoned,
        WorkflowCoordinatorError::RecoveryRequired,
        WorkflowCoordinatorError::RecoveryInvalid,
        WorkflowCoordinatorError::RecoveryConflict,
        WorkflowCoordinatorError::JournalUnavailable,
        WorkflowCoordinatorError::JournalLimit,
        WorkflowCoordinatorError::CleanupTimeout,
        WorkflowCoordinatorError::WaitTimeoutExceeded,
        WorkflowCoordinatorError::InvalidRunId,
        WorkflowCoordinatorError::TrustedLimitExceeded("max_nodes"),
        WorkflowCoordinatorError::InvalidGeneratedRunId("bad/id".into()),
        WorkflowCoordinatorError::InvalidGeneratedAttemptId(WorkflowAttemptId::new("bad/id")),
        WorkflowCoordinatorError::AttemptIdCollision(WorkflowAttemptId::new("watt_same")),
        WorkflowCoordinatorError::InvalidExecutionLease,
        WorkflowCoordinatorError::StaleExecutionLease,
        WorkflowCoordinatorError::RunDeadlineExceeded,
        WorkflowCoordinatorError::UnsupportedWorkerProfile {
            profile: WorkflowWorkerProfileRef::new("custom"),
        },
        WorkflowCoordinatorError::UnsupportedWorkerProfileForNode {
            node_id: WorkflowNodeId::new("source"),
            profile: WorkflowWorkerProfileRef::new("custom"),
        },
        WorkflowCoordinatorError::RunIdCollision("wrun_same".into()),
        WorkflowCoordinatorError::Request(WorkflowRequestError::InvalidRequestId),
        WorkflowCoordinatorError::Validation(WorkflowValidationError::EmptyGoal),
        WorkflowCoordinatorError::Schema(WorkflowSchemaError::InvalidSchema),
        WorkflowCoordinatorError::Reducer(WorkflowReduceError::WrongRun),
        WorkflowCoordinatorError::UnexpectedStaleEvent,
        WorkflowCoordinatorError::Encoding("encoding failed".into()),
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
        let _: &dyn std::error::Error = &error;
    }
    assert!(matches!(
        WorkflowCoordinatorError::from(WorkflowRequestError::InvalidRequestId),
        WorkflowCoordinatorError::Request(_)
    ));
    assert!(matches!(
        WorkflowCoordinatorError::from(WorkflowValidationError::EmptyGoal),
        WorkflowCoordinatorError::Validation(_)
    ));
    assert!(matches!(
        WorkflowCoordinatorError::from(WorkflowSchemaError::InvalidSchema),
        WorkflowCoordinatorError::Schema(_)
    ));
    assert!(matches!(
        WorkflowCoordinatorError::from(WorkflowReduceError::WrongRun),
        WorkflowCoordinatorError::Reducer(_)
    ));
    assert_eq!(
        WorkflowCoordinatorError::from(loopal_storage::WorkflowJournalError::LimitExceeded {
            limit: loopal_storage::WorkflowJournalLimit::Entries,
            actual: 2,
            max: 1,
        }),
        WorkflowCoordinatorError::JournalLimit
    );
    assert_eq!(
        WorkflowCoordinatorError::from(loopal_storage::WorkflowJournalError::Io {
            path: "journal.jsonl".into(),
            source: std::io::Error::other("unavailable"),
        }),
        WorkflowCoordinatorError::JournalUnavailable
    );
}

#[test]
fn saturated_revision_exposes_unexpected_stale_guard() {
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_saturated"),
        QualifiedAddress::local("root"),
        super::support::spec(),
        1,
    );
    run.revision = u64::MAX;
    assert_eq!(
        apply_payload(&run, WorkflowEventPayload::SpecValidated, 2).map(|(_, run)| run),
        Err(WorkflowCoordinatorError::UnexpectedStaleEvent)
    );
}

#[tokio::test]
async fn invalid_get_inputs_fail_before_lookup() {
    let (handle, task, _, _) = coordinator(WorkflowCoordinatorMode::Preview, [], []);
    assert_eq!(
        handle
            .get(
                owner("bad/session", "root"),
                get_request("wreq_get_owner", WorkflowRunId::new("wrun_valid")),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidOwner)
    );
    assert_eq!(
        handle
            .get(
                owner("session", "root"),
                get_request("wreq_get_run", WorkflowRunId::new("bad/id")),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidRunId)
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn closed_actor_and_dropped_responses_report_unavailable() {
    let (handle, task) = WorkflowCoordinator::spawn_for_test(
        WorkflowCoordinatorMode::Preview,
        Arc::new(TestClock::new([])),
        Arc::new(TestIds::new([])),
        Arc::new(TestJournal::new()),
    );
    task.abort();
    let _ = task.await;
    assert_eq!(
        handle
            .start(owner("session", "root"), request("wreq_closed"))
            .await,
        Err(WorkflowCoordinatorError::Unavailable)
    );

    let (commands, mut receiver) = mpsc::channel(1);
    let handle = WorkflowCoordinatorHandle { commands };
    let drop_response = tokio::spawn(async move {
        if let Some(WorkflowCommand::Get { response, .. }) = receiver.recv().await {
            drop(response);
        }
    });
    assert_eq!(
        handle
            .get(
                owner("session", "root"),
                get_request("wreq_get_missing", WorkflowRunId::new("wrun_missing")),
            )
            .await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
    drop_response.await.unwrap();
}
