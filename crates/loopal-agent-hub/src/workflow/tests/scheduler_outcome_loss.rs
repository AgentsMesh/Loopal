use loopal_protocol::{WorkflowFailureClass, WorkflowRunState};

use super::scheduler_outcome_loss_support::running_attempt;
use super::scheduler_support::SpawnerEffect;
use crate::workflow::scheduler::{
    WorkflowCleanupStatus, WorkflowStopStatus, WorkflowWorkerOutcome,
};

#[tokio::test]
async fn dropping_last_coordinator_handle_releases_live_outcome_waiter() {
    let mut fixture = running_attempt("drop-live-outcome", 35).await;
    let outcome = fixture.outcome.take().unwrap();
    let task = fixture.task;
    drop(fixture.handle);
    let SpawnerEffect::Shutdown { response, .. } = fixture.control.next().await else {
        panic!("expected shutdown during coordinator drain")
    };
    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());
    tokio::time::timeout(std::time::Duration::from_secs(1), task)
        .await
        .expect("coordinator must stop when the last handle is dropped")
        .unwrap();
    assert!(outcome.is_closed());
}

#[tokio::test]
async fn outcome_before_shutdown_confirmation_does_not_mask_cleanup_status() {
    let mut fixture = running_attempt("outcome-before-shutdown", 34).await;
    fixture.cancel().await;
    let SpawnerEffect::Interrupt { response, .. } = fixture.control.next().await else {
        panic!("expected cancellation interrupt")
    };
    assert!(response.send(WorkflowStopStatus::Requested).is_ok());
    fixture.handle.tick(10_000).await.unwrap();
    let SpawnerEffect::Shutdown {
        response,
        execution,
        ..
    } = fixture.control.next().await
    else {
        panic!("expected shutdown escalation")
    };

    fixture
        .outcome
        .take()
        .unwrap()
        .send(WorkflowWorkerOutcome::Succeeded {
            completion: loopal_protocol::AgentCompletion::goal(Some("late success".into())),
            output: None,
        })
        .unwrap();
    assert_eq!(
        fixture.run("outcome_before_cleanup").await.state,
        WorkflowRunState::Cancelling
    );

    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());
    fixture.journal.wait_for_event_batches(6).await;
    assert_eq!(
        fixture.run("outcome_after_cleanup").await.state,
        WorkflowRunState::Cancelled
    );
    assert_eq!(execution, fixture.execution);
    fixture.finish().await;
}

#[tokio::test]
async fn lost_running_outcome_durably_stops_and_cleans_exact_lease() {
    let mut fixture = running_attempt("running", 31).await;
    drop(fixture.outcome.take());
    let SpawnerEffect::Interrupt {
        execution,
        response,
    } = fixture.control.next().await
    else {
        panic!("expected interrupt after outcome loss")
    };
    assert_eq!(execution, fixture.execution);
    assert_eq!(fixture.journal.events().len(), 5);
    assert!(response.send(WorkflowStopStatus::Requested).is_ok());

    fixture.handle.tick(10_000).await.unwrap();
    let SpawnerEffect::Shutdown {
        execution,
        response,
        ..
    } = fixture.control.next().await
    else {
        panic!("expected shutdown after outcome-loss grace")
    };
    assert_eq!(execution, fixture.execution);
    assert_eq!(
        fixture.run("during_cleanup").await.state,
        WorkflowRunState::Running
    );
    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());
    fixture.journal.wait_for_event_batches(6).await;

    let run = fixture.run("after_cleanup").await;
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(
        run.failure.unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    fixture.finish().await;
}

#[tokio::test]
async fn lost_interrupting_outcome_waits_for_existing_stop_confirmation() {
    let mut fixture = running_attempt("interrupting", 32).await;
    fixture.cancel().await;
    let SpawnerEffect::Interrupt { response, .. } = fixture.control.next().await else {
        panic!("expected cancellation interrupt")
    };
    drop(fixture.outcome.take());
    fixture.deliver_lost_barrier().await;
    assert_eq!(
        fixture.run("before_stop").await.state,
        WorkflowRunState::Cancelling
    );
    assert_eq!(fixture.journal.events().len(), 5);

    assert!(response.send(WorkflowStopStatus::Stopped).is_ok());
    fixture.journal.wait_for_event_batches(6).await;
    assert_eq!(
        fixture.run("after_stop").await.state,
        WorkflowRunState::Cancelled
    );
    fixture.finish().await;
}

#[tokio::test]
async fn lost_shutting_down_outcome_waits_for_existing_cleanup_confirmation() {
    let mut fixture = running_attempt("shutdown", 33).await;
    fixture.cancel().await;
    let SpawnerEffect::Interrupt { response, .. } = fixture.control.next().await else {
        panic!("expected cancellation interrupt")
    };
    assert!(response.send(WorkflowStopStatus::Requested).is_ok());
    fixture.handle.tick(10_000).await.unwrap();
    let SpawnerEffect::Shutdown {
        execution,
        response,
        ..
    } = fixture.control.next().await
    else {
        panic!("expected escalated shutdown")
    };
    assert_eq!(execution, fixture.execution);
    drop(fixture.outcome.take());
    fixture.deliver_lost_barrier().await;
    assert_eq!(
        fixture.run("before_cleanup").await.state,
        WorkflowRunState::Cancelling
    );
    assert_eq!(fixture.journal.events().len(), 5);

    assert!(response.send(WorkflowCleanupStatus::Confirmed).is_ok());
    fixture.journal.wait_for_event_batches(6).await;
    assert_eq!(
        fixture.run("after_cleanup").await.state,
        WorkflowRunState::Cancelled
    );
    fixture.finish().await;
}
