use loopal_protocol::WorkflowRunId;

use super::super::{WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::support::{coordinator_with_journal, owner, request};

#[tokio::test]
async fn start_is_journaled_once_before_commit_and_replay() {
    let run_id = WorkflowRunId::new("wrun_started");
    let (handle, task, _, _, journal) =
        coordinator_with_journal(WorkflowCoordinatorMode::Preview, [10, 11], [run_id.clone()]);
    let owner = owner("session", "root");
    let request = request("wreq_start");

    let first = handle.start(owner.clone(), request.clone()).await.unwrap();
    let replay = handle.start(owner.clone(), request).await.unwrap();

    assert_eq!(replay, first);
    let starts = journal.starts();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].owner, owner);
    assert_eq!(starts[0].planned.id, run_id);
    assert_eq!(starts[0].planned.revision, 0);
    assert_eq!(starts[0].event.revision, 1);
    assert_eq!(starts[0].request.operation, "start");
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn start_append_error_poisons_owner_without_committing_run() {
    let (handle, task, _, ids, journal) = coordinator_with_journal(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [WorkflowRunId::new("wrun_uncommitted")],
    );
    let owner = owner("session", "root");
    let request = request("wreq_start");
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);

    assert_eq!(
        handle.start(owner.clone(), request.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert!(journal.starts().is_empty());
    assert_eq!(ids.calls(), 1);
    assert_eq!(
        handle.start(owner.clone(), request).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    assert_eq!(
        handle.recover(owner).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    assert_eq!(ids.calls(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn append_task_failure_also_poisons_owner() {
    let (handle, task, _, ids, journal) = coordinator_with_journal(
        WorkflowCoordinatorMode::Preview,
        [10, 11],
        [WorkflowRunId::new("wrun_ambiguous")],
    );
    let owner = owner("session", "root");
    let request = request("wreq_start");
    journal.push_append_panic();

    assert_eq!(
        handle.start(owner.clone(), request.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(ids.calls(), 1);
    assert_eq!(
        handle.start(owner, request).await,
        Err(WorkflowCoordinatorError::OwnerPoisoned)
    );
    assert_eq!(ids.calls(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn configured_ceiling_rejection_does_not_consume_id_or_write_journal() {
    use std::sync::Arc;

    use super::super::WorkflowCoordinator;
    use super::super::actor::{WorkflowRuntimeConfig, WorkflowTrustedCeilings};
    use super::journal_support::TestJournal;
    use super::support::{TestClock, TestIds};

    let clock = Arc::new(TestClock::new([]));
    let ids = Arc::new(TestIds::new([WorkflowRunId::new("wrun_unused")]));
    let journal = Arc::new(TestJournal::new());
    let (handle, task) = WorkflowCoordinator::spawn_with_runtime_config(
        WorkflowCoordinatorMode::Preview,
        clock.clone(),
        ids.clone(),
        journal.clone(),
        Arc::new(super::super::scheduler::UnavailableWorkflowSpawner),
        None,
        WorkflowRuntimeConfig {
            ceilings: WorkflowTrustedCeilings {
                max_nodes: 1,
                max_parallel: 2,
                max_attempts: 8,
                max_output_bytes: 4_096,
                run_deadline_ms: 60_000,
                attempt_timeout_ms: 30_000,
            },
            cancel_grace_ms: 17_000,
            recovery_grace_ms: 0,
            redaction_seed: loopal_output_guard::FinalSinkRedactionSeed::new(),
        },
    );

    assert_eq!(
        handle
            .start(owner("session", "root"), request("wreq_too_many_nodes"))
            .await,
        Err(WorkflowCoordinatorError::TrustedLimitExceeded("max_nodes"))
    );
    assert_eq!(ids.calls(), 0);
    assert_eq!(clock.calls(), 0);
    assert!(journal.starts().is_empty());
    drop(handle);
    task.await.unwrap();
}

#[test]
fn settings_convert_to_millisecond_runtime_ceilings() {
    use super::super::actor::WorkflowTrustedCeilings;

    let mut settings = loopal_config::WorkflowSettings::default();
    settings.limits.max_nodes = 9;
    settings.limits.max_parallel = 3;
    settings.limits.max_attempts = 18;
    settings.limits.max_output_bytes = 12_345;
    settings.timing.run_deadline_secs = 77;
    settings.timing.attempt_timeout_secs = 13;
    let ceilings = WorkflowTrustedCeilings::from_settings(&settings);
    assert_eq!(ceilings.max_nodes, 9);
    assert_eq!(ceilings.max_parallel, 3);
    assert_eq!(ceilings.max_attempts, 18);
    assert_eq!(ceilings.max_output_bytes, 12_345);
    assert_eq!(ceilings.run_deadline_ms, 77_000);
    assert_eq!(ceilings.attempt_timeout_ms, 13_000);
}

#[test]
fn every_trusted_ceiling_is_enforced() {
    use super::super::actor::WorkflowTrustedCeilings;

    let ceiling = WorkflowTrustedCeilings {
        max_nodes: 8,
        max_parallel: 2,
        max_attempts: 8,
        max_output_bytes: 4_096,
        run_deadline_ms: 60_000,
        attempt_timeout_ms: 30_000,
    };
    let base = request("wreq_limits").spec.limits;
    assert!(ceiling.validate(&base).is_ok());
    let mut cases = Vec::new();
    let mut limits = base.clone();
    limits.max_nodes += 1;
    cases.push((limits, "max_nodes"));
    let mut limits = base.clone();
    limits.max_parallel += 1;
    cases.push((limits, "max_parallel"));
    let mut limits = base.clone();
    limits.max_attempts += 1;
    cases.push((limits, "max_attempts"));
    let mut limits = base.clone();
    limits.max_output_bytes += 1;
    cases.push((limits, "max_output_bytes"));
    let mut limits = base.clone();
    limits.run_deadline_ms += 1;
    cases.push((limits, "run_deadline_ms"));
    let mut limits = base;
    limits.attempt_timeout_ms += 1;
    cases.push((limits, "attempt_timeout_ms"));
    for (limits, field) in cases {
        assert_eq!(
            ceiling.validate(&limits),
            Err(WorkflowCoordinatorError::TrustedLimitExceeded(field))
        );
    }
}
