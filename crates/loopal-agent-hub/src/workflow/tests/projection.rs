use std::sync::Arc;

use loopal_protocol::{
    AgentEventPayload, QualifiedAddress, WorkflowRunId, WorkflowRunSnapshot, WorkflowRunState,
};
use tokio::sync::mpsc;

use super::super::recovery::RecoveredOwner;
use super::super::{WorkflowCoordinator, WorkflowCoordinatorMode};
use super::journal_support::TestJournal;
use super::support::{TestClock, TestIds, owner, request, spec};

#[tokio::test]
async fn durable_start_publishes_authoritative_summary() {
    let (events, mut receiver) = mpsc::channel(4);
    let journal = Arc::new(TestJournal::new());
    let (handle, task) = WorkflowCoordinator::spawn_for_test_with_events(
        WorkflowCoordinatorMode::Preview,
        Arc::new(TestClock::new([10, 11])),
        Arc::new(TestIds::new([WorkflowRunId::new("wrun_event")])),
        journal.clone(),
        events,
    );

    let response = handle
        .start(owner("session", "root"), request("wreq_event"))
        .await
        .unwrap();
    let event = receiver.recv().await.unwrap();
    assert_eq!(event.agent_name, Some(QualifiedAddress::local("root")));
    let AgentEventPayload::WorkflowRunChanged(summary) = event.payload else {
        panic!("expected workflow projection event")
    };
    assert_eq!(summary, response.summary);
    assert_eq!(journal.starts().len(), 1);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn closed_projection_queue_does_not_rewrite_durable_start_outcome() {
    let (events, receiver) = mpsc::channel(1);
    drop(receiver);
    let journal = Arc::new(TestJournal::new());
    let workflow_owner = owner("session", "root");
    let (handle, task) = WorkflowCoordinator::spawn_for_test_with_events(
        WorkflowCoordinatorMode::Preview,
        Arc::new(TestClock::new([10, 11])),
        Arc::new(TestIds::new([WorkflowRunId::new("wrun_closed")])),
        journal.clone(),
        events,
    );

    let request = request("wreq_closed_sink");
    let response = handle
        .start(workflow_owner.clone(), request.clone())
        .await
        .unwrap();
    assert_eq!(
        handle.start(workflow_owner.clone(), request).await.unwrap(),
        response
    );
    assert_eq!(journal.starts().len(), 1);
    let snapshot = handle.snapshot(workflow_owner).await.unwrap();
    assert_eq!(snapshot.active, vec![response.summary]);
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn saturated_projection_queue_is_best_effort() {
    let (events, _receiver) = mpsc::channel(1);
    events
        .try_send(loopal_protocol::AgentEvent::named(
            QualifiedAddress::local("root"),
            AgentEventPayload::Running,
        ))
        .unwrap();
    let journal = Arc::new(TestJournal::new());
    let workflow_owner = owner("session", "root");
    let (handle, task) = WorkflowCoordinator::spawn_for_test_with_events(
        WorkflowCoordinatorMode::Preview,
        Arc::new(TestClock::new([10, 11])),
        Arc::new(TestIds::new([WorkflowRunId::new("wrun_full")])),
        journal.clone(),
        events,
    );

    let response = handle
        .start(workflow_owner.clone(), request("wreq_full_sink"))
        .await
        .unwrap();
    assert_eq!(journal.starts().len(), 1);
    assert_eq!(
        handle.snapshot(workflow_owner).await.unwrap().active,
        vec![response.summary]
    );
    drop(handle);
    task.await.unwrap();
}

#[tokio::test]
async fn recovery_publishes_seed_and_snapshot_is_owner_scoped_and_bounded() {
    let (events, mut receiver) = mpsc::channel(64);
    let journal = Arc::new(TestJournal::new());
    let workflow_owner = owner("session", "root");
    let mut runs = Vec::new();
    for index in 0..35 {
        runs.push(run(
            &format!("wrun_terminal_{index:02}"),
            WorkflowRunState::Succeeded,
            100 + index,
        ));
    }
    runs.push(run("wrun_active_old", WorkflowRunState::Running, 5));
    runs.push(run("wrun_active_new", WorkflowRunState::Validated, 500));
    journal.push_recovery(Ok(RecoveredOwner {
        runs,
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    }));
    let (handle, task) = WorkflowCoordinator::spawn_for_test_with_events(
        WorkflowCoordinatorMode::Preview,
        Arc::new(TestClock::new([])),
        Arc::new(TestIds::new([])),
        journal,
        events,
    );

    assert_eq!(handle.recover(workflow_owner.clone()).await.unwrap(), 37);
    for _ in 0..37 {
        assert!(matches!(
            receiver.recv().await.unwrap().payload,
            AgentEventPayload::WorkflowRunChanged(_)
        ));
    }
    let snapshot = handle.snapshot(workflow_owner).await.unwrap();
    assert_eq!(snapshot.active.len(), 2);
    assert_eq!(snapshot.active[0].id.as_str(), "wrun_active_new");
    assert_eq!(snapshot.active[1].id.as_str(), "wrun_active_old");
    assert_eq!(snapshot.recent.len(), 32);
    assert_eq!(snapshot.recent[0].id.as_str(), "wrun_terminal_34");
    assert_eq!(snapshot.recent[31].id.as_str(), "wrun_terminal_03");
    assert!(
        handle
            .snapshot(owner("other-session", "root"))
            .await
            .unwrap()
            .is_empty()
    );
    drop(handle);
    task.await.unwrap();
}

fn run(id: &str, state: WorkflowRunState, updated_at_unix_ms: u64) -> WorkflowRunSnapshot {
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new(id),
        QualifiedAddress::local("root"),
        spec(),
        1,
    );
    run.state = state;
    run.revision = 1;
    run.updated_at_unix_ms = updated_at_unix_ms;
    run
}
