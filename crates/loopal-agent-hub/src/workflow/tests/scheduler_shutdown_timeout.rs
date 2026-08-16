use std::time::Duration;

use loopal_protocol::{
    WorkflowAttemptState, WorkflowEventPayload, WorkflowFailureClass, WorkflowRunState,
};

use super::scheduler_outcome_loss_support::running_attempt;
use super::scheduler_support::SpawnerEffect;
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::scheduler::WorkflowStopStatus;

#[tokio::test(start_paused = true)]
async fn hung_active_shutdown_times_out_and_fails_closed_without_retry() {
    let mut fixture = running_attempt("shutdown-timeout", 47).await;
    fixture.deliver_lost_barrier().await;
    let SpawnerEffect::Interrupt {
        execution,
        response,
    } = fixture.control.next().await
    else {
        panic!("expected outcome-loss interrupt")
    };
    assert_eq!(execution, fixture.execution);
    assert!(response.send(WorkflowStopStatus::Requested).is_ok());

    fixture.handle.tick(10_000).await.unwrap();
    let SpawnerEffect::Shutdown {
        execution,
        mut response,
    } = fixture.control.next().await
    else {
        panic!("expected escalated shutdown")
    };
    assert_eq!(execution, fixture.execution);
    assert_eq!(
        fixture.run("during_hung_shutdown").await.state,
        WorkflowRunState::Running
    );

    tokio::time::advance(Duration::from_secs(5)).await;
    tokio::time::timeout(Duration::from_millis(1), response.closed())
        .await
        .expect("outer shutdown timeout must drop the hung adapter future");
    assert!(response.is_closed());
    fixture.journal.wait_for_event_batches(6).await;

    {
        let batches = fixture.journal.events();
        let event = batches
            .last()
            .and_then(|(_, _, events)| events.last())
            .expect("cleanup timeout must append a terminal event");
        let WorkflowEventPayload::AttemptFailed {
            completion,
            failure,
            ..
        } = &event.payload
        else {
            panic!("cleanup timeout must durably fail the attempt")
        };
        assert_eq!(completion.reason, "workflow_cleanup_timeout");
        assert_eq!(failure.class, WorkflowFailureClass::AmbiguousExecution);
    }

    let run = fixture.run("after_shutdown_timeout").await;
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.attempts.len(), 1, "cleanup timeout must not retry");
    assert_eq!(
        run.failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    let attempt = &run.attempts[0];
    assert_eq!(attempt.state, WorkflowAttemptState::Failed);
    assert_eq!(
        attempt.failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    assert_eq!(
        attempt.completion.as_ref().unwrap().reason,
        "workflow_cleanup_timeout"
    );
    assert!(fixture.outcome.take().unwrap().is_closed());
    fixture.control.assert_idle().await;

    fixture.handle.shutdown().await.unwrap();
    fixture.task.await.unwrap();
    fixture.control.assert_drained().await;
}

#[tokio::test(start_paused = true)]
async fn coordinator_shutdown_timeout_terminalizes_active_lease_and_reports_error() {
    let mut fixture = running_attempt("drain-timeout", 53).await;
    let shutdown_handle = fixture.handle.clone();
    let shutdown = tokio::spawn(async move { shutdown_handle.shutdown().await });
    let SpawnerEffect::Shutdown {
        execution,
        mut response,
    } = fixture.control.next().await
    else {
        panic!("expected active cleanup during coordinator shutdown")
    };
    assert_eq!(execution, fixture.execution);

    tokio::time::advance(Duration::from_secs(5)).await;
    tokio::time::timeout(Duration::from_millis(1), response.closed())
        .await
        .expect("bounded cleanup must release a hung shutdown request");
    assert!(response.is_closed());

    assert_eq!(
        shutdown.await.unwrap(),
        Err(WorkflowCoordinatorError::CleanupTimeout)
    );
    fixture.journal.wait_for_event_batches(5).await;
    let start = fixture.journal.starts().first().unwrap().clone();
    let mut run = crate::workflow::transition::apply_event(&start.planned, &start.event).unwrap();
    for event in fixture
        .journal
        .events()
        .into_iter()
        .flat_map(|(_, _, events)| events)
    {
        run = crate::workflow::transition::apply_event(&run, &event).unwrap();
    }
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.attempts.len(), 1);
    assert_eq!(
        run.attempts[0].completion.as_ref().unwrap().reason,
        "workflow_cleanup_timeout"
    );
    assert_eq!(
        run.attempts[0].failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    assert!(fixture.outcome.take().unwrap().is_closed());
    fixture.task.await.unwrap();
    fixture.control.assert_drained().await;
}
