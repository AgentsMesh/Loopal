use std::sync::Arc;

use loopal_protocol::{WorkflowFailureClass, WorkflowRunId, WorkflowRunState};

use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::recovery::RecoveredOwner;
use crate::workflow::tests::journal_support::TestJournal;
use crate::workflow::tests::scheduler_recovery::{RecoveredAttempt, recover_case, recovered_run};
use crate::workflow::tests::scheduler_support::{coordinator, test_spawner};
use crate::workflow::tests::support::{get_request, owner};

#[tokio::test]
async fn recovery_fails_unbound_dispatch_without_retry_or_spawn() {
    let run = recovered_run(RecoveredAttempt::Unbound);
    let recovered = recover_case(run, "unbound").await;
    assert_eq!(recovered.state, WorkflowRunState::Failed);
    assert_eq!(
        recovered.failure.unwrap().class,
        WorkflowFailureClass::Permanent
    );
}

#[tokio::test]
async fn recovery_marks_bound_and_running_attempts_ambiguous() {
    for (attempt, suffix) in [
        (RecoveredAttempt::Bound, "bound"),
        (RecoveredAttempt::Running, "running"),
    ] {
        let recovered = recover_case(recovered_run(attempt), suffix).await;
        assert_eq!(recovered.state, WorkflowRunState::Failed);
        assert_eq!(
            recovered.failure.unwrap().class,
            WorkflowFailureClass::AmbiguousExecution
        );
    }
}

#[tokio::test]
async fn recovery_fails_cancelling_run_without_restarting_worker() {
    let recovered = recover_case(recovered_run(RecoveredAttempt::Cancelling), "cancelling").await;
    assert_eq!(recovered.state, WorkflowRunState::Failed);
    assert_eq!(
        recovered.failure.unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
}

#[tokio::test]
async fn recovery_append_failure_poisons_owner() {
    let journal = Arc::new(TestJournal::new());
    let owner = owner("session-recovery-fail", "root");
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![recovered_run(RecoveredAttempt::Running)],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    let (spawner, control) = test_spawner();
    let (handle, task, _, _) = coordinator([900], [], [], journal.clone(), spawner);

    assert_eq!(
        handle.recover(owner.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        handle.recover(owner.clone()).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    assert_eq!(
        handle
            .get(
                owner,
                get_request("wreq_poisoned_get", WorkflowRunId::new("wrun_recovered")),
            )
            .await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    assert!(journal.events().is_empty());
    control.assert_idle().await;
    drop(handle);
    task.await.unwrap();
}
