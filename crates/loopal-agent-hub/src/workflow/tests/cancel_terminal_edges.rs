use std::sync::Arc;

use loopal_protocol::{
    WorkflowCancelRequest, WorkflowRequestId, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState,
    WorkflowTerminalDeliveryId, WorkflowTerminalDisposition,
};

use super::super::command::WorkflowCommand;
use super::super::recovery::RecoveredOwner;
use super::super::terminal_delivery::{
    UnavailableWorkflowTerminalSink, WorkflowTerminalSink, payload,
};
use super::super::{WorkflowCoordinator, WorkflowCoordinatorError, WorkflowCoordinatorMode};
use super::journal_support::TestJournal;
use super::support::{
    TestClock, TestIds, coordinator, coordinator_with_journal, owner, request, spec,
};

fn cancel_request(
    id: &str,
    run_id: impl Into<WorkflowRunId>,
    reason: Option<&str>,
) -> WorkflowCancelRequest {
    WorkflowCancelRequest {
        request_id: WorkflowRequestId::new(id),
        run_id: run_id.into(),
        reason: reason.map(str::to_owned),
    }
}

#[tokio::test]
async fn cancel_rejects_disabled_and_invalid_authority_before_lookup() {
    let (disabled, disabled_task) = WorkflowCoordinator::spawn_disabled();
    assert_eq!(
        disabled
            .cancel(
                owner("session", "root"),
                cancel_request("wreq_disabled", "wrun_valid", None),
            )
            .await,
        Err(WorkflowCoordinatorError::Disabled)
    );
    disabled.shutdown().await.unwrap();
    disabled_task.await.unwrap();

    let (handle, task, _, _) = coordinator(WorkflowCoordinatorMode::Preview, [], []);
    assert_eq!(
        handle
            .cancel(
                owner("bad/session", "root"),
                cancel_request("wreq_bad_owner", "wrun_valid", None),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidOwner)
    );
    assert_eq!(
        handle
            .cancel(
                owner("session", "root"),
                cancel_request("wreq_bad_run", "bad/id", None),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidRunId)
    );
    assert_eq!(
        handle
            .cancel(
                owner("session", "root"),
                cancel_request("wreq_missing_run", "wrun_missing", None),
            )
            .await,
        Err(WorkflowCoordinatorError::InvalidRunId)
    );
    handle
        .commands
        .send(WorkflowCommand::TerminalDeliveryResolved {
            owner: owner("session", "root"),
            delivery_id: WorkflowTerminalDeliveryId::new(
                "session",
                WorkflowRunId::new("wrun_unknown_delivery"),
                1,
            ),
            result: Ok(WorkflowTerminalDisposition::Applied),
            task_panicked: false,
        })
        .await
        .unwrap();
    handle.tick(0).await.unwrap();
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn cancel_replays_exact_requests_and_noops_after_terminal_state() {
    let run_id = WorkflowRunId::new("wrun_cancel_edges");
    let (handle, task, _, _, _) = coordinator_with_journal(
        WorkflowCoordinatorMode::Preview,
        [10, 11, 12],
        [run_id.clone()],
    );
    let workflow_owner = owner("session", "root");
    handle.recover(workflow_owner.clone()).await.unwrap();
    handle
        .start(workflow_owner.clone(), request("wreq_start_cancel_edges"))
        .await
        .unwrap();
    let first_request = cancel_request("wreq_cancel_edges", run_id.clone(), None);
    let first = handle
        .cancel(workflow_owner.clone(), first_request.clone())
        .await
        .unwrap();
    assert!(!first.already_terminal);
    assert_eq!(
        handle
            .cancel(workflow_owner.clone(), first_request)
            .await
            .unwrap(),
        first
    );

    let no_op = handle
        .cancel(
            workflow_owner,
            cancel_request("wreq_cancel_terminal", run_id, Some("already done")),
        )
        .await
        .unwrap();
    assert!(no_op.already_terminal);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn terminal_activation_fails_closed_when_intent_persistence_fails() {
    let workflow_owner = owner("session", "root");
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_missing_intent"),
        workflow_owner.root_agent.clone(),
        spec(),
        1,
    );
    run.state = WorkflowRunState::Cancelled;
    run.revision = 2;
    let notification = payload::from_snapshot(
        &workflow_owner,
        &run,
        &loopal_output_guard::FinalSinkRedactionSeed::new(),
    )
    .unwrap();
    assert_eq!(
        UnavailableWorkflowTerminalSink
            .deliver(&workflow_owner, notification)
            .await,
        Err("workflow terminal sink is unavailable".into())
    );

    let journal = Arc::new(TestJournal::new());
    journal.push_recovery(Ok(RecoveredOwner {
        runs: vec![run],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (handle, task) = WorkflowCoordinator::spawn_for_test(
        WorkflowCoordinatorMode::Preview,
        Arc::new(TestClock::new([])),
        Arc::new(TestIds::new([])),
        journal.clone(),
    );
    assert_eq!(
        handle
            .activate_terminal_deliveries(workflow_owner.clone())
            .await,
        Err(WorkflowCoordinatorError::RecoveryRequired)
    );
    handle.recover(workflow_owner.clone()).await.unwrap();
    journal.push_append_error(WorkflowCoordinatorError::JournalUnavailable);
    assert_eq!(
        handle
            .activate_terminal_deliveries(workflow_owner.clone())
            .await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert_eq!(
        handle.activate_terminal_deliveries(workflow_owner).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    drop(handle);
    task.await.unwrap();
}
