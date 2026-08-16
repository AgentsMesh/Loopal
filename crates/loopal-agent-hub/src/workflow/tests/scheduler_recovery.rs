use std::sync::Arc;

use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptCapability, WorkflowAttemptId, WorkflowEventPayload,
    WorkflowRunId, WorkflowRunSnapshot,
};

use super::journal_support::TestJournal;
use super::scheduler_support::{coordinator, test_spawner};
use super::support::{get_request, owner, spec};
use crate::workflow::recovery::RecoveredOwner;
use crate::workflow::transition::apply_payload;

#[path = "scheduler_recovery_cases.rs"]
mod cases;
#[path = "scheduler_recovery_resume.rs"]
mod resume;

pub(super) async fn recover_case(run: WorkflowRunSnapshot, suffix: &str) -> WorkflowRunSnapshot {
    let journal = Arc::new(TestJournal::new());
    let owner = owner(&format!("session-recovery-{suffix}"), "root");
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![run],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (spawner, control) = test_spawner();
    let (handle, task, clock, ids) = coordinator([800], [], [], journal.clone(), spawner);

    assert_eq!(handle.recover(owner.clone()).await.unwrap(), 1);
    let recovered = handle
        .get(
            owner,
            get_request(
                &format!("wreq_recovered_{suffix}"),
                WorkflowRunId::new("wrun_recovered"),
            ),
        )
        .await
        .unwrap()
        .run
        .unwrap();
    assert_eq!(clock.calls(), 1);
    assert_eq!(ids.calls(), 0);
    assert_eq!(ids.attempt_calls(), 0);
    assert_eq!(journal.events().len(), 1);
    assert!(matches!(
        journal.events()[0].2[0].payload,
        WorkflowEventPayload::AttemptFailed { .. }
    ));
    control.assert_idle().await;
    drop(handle);
    task.await.unwrap();
    recovered
}

#[derive(Clone, Copy)]
pub(super) enum RecoveredAttempt {
    Unbound,
    Bound,
    Running,
    Cancelling,
}

pub(super) fn recovered_run(kind: RecoveredAttempt) -> WorkflowRunSnapshot {
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_recovered"),
        QualifiedAddress::local("root"),
        spec(),
        1,
    );
    run = apply(run, WorkflowEventPayload::SpecValidated);
    run = apply(run, WorkflowEventPayload::RunStarted);
    let attempt_id = WorkflowAttemptId::new("watt_recovered");
    run = apply(
        run,
        WorkflowEventPayload::DispatchIntended {
            node_id: "source".into(),
            attempt_id: attempt_id.clone(),
            capability_digest: WorkflowAttemptCapability::parse("11".repeat(32))
                .unwrap()
                .digest(),
        },
    );
    if !matches!(kind, RecoveredAttempt::Unbound) {
        run = apply(
            run,
            WorkflowEventPayload::AttemptBound {
                node_id: "source".into(),
                attempt_id: attempt_id.clone(),
                agent: QualifiedAddress::local("worker"),
            },
        );
    }
    if matches!(
        kind,
        RecoveredAttempt::Running | RecoveredAttempt::Cancelling
    ) {
        run = apply(
            run,
            WorkflowEventPayload::AttemptRunning {
                node_id: "source".into(),
                attempt_id,
            },
        );
    }
    if matches!(kind, RecoveredAttempt::Cancelling) {
        run = apply(
            run,
            WorkflowEventPayload::CancelRequested {
                reason: Some("restart during cancellation".into()),
            },
        );
    }
    run
}

pub(super) fn recovered_quiescent_run(run_id: WorkflowRunId, running: bool) -> WorkflowRunSnapshot {
    let mut run = WorkflowRunSnapshot::planned(run_id, QualifiedAddress::local("root"), spec(), 1);
    run = apply(run, WorkflowEventPayload::SpecValidated);
    if running {
        run = apply(run, WorkflowEventPayload::RunStarted);
    }
    run
}

pub(super) async fn shutdown_with_pending(
    handle: crate::workflow::WorkflowCoordinatorHandle,
    task: tokio::task::JoinHandle<()>,
    control: super::scheduler_support::SpawnerControl,
    _prepare: tokio::sync::oneshot::Sender<
        Result<
            crate::workflow::scheduler::WorkflowPreparedWorker,
            crate::workflow::scheduler::WorkflowSpawnFailure,
        >,
    >,
) {
    let shutdown = tokio::spawn({
        let handle = handle.clone();
        async move { handle.shutdown().await }
    });
    let super::scheduler_support::SpawnerEffect::AbortPrepare { response, .. } =
        control.next().await
    else {
        panic!("expected pending preparation abort during shutdown")
    };
    let _ = response.send(crate::workflow::scheduler::WorkflowCleanupStatus::Confirmed);
    shutdown.await.unwrap().unwrap();
    task.await.unwrap();
    control.assert_drained().await;
}

fn apply(run: WorkflowRunSnapshot, payload: WorkflowEventPayload) -> WorkflowRunSnapshot {
    let occurred_at = run.updated_at_unix_ms.saturating_add(1);
    apply_payload(&run, payload, occurred_at).unwrap().1
}
