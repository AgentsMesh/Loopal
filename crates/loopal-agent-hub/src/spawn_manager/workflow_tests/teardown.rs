use std::time::Duration;

use loopal_protocol::AgentCompletion;

use super::support::harness;
use crate::workflow::scheduler::{WorkflowCleanupStatus, WorkflowSpawner};

#[tokio::test]
async fn exact_finish_reports_an_unconfirmed_generation_detach() {
    let fixture = harness().await;
    let control = fixture.spawner.attempts.lock().await.by_attempt[&fixture.causation.attempt_id]
        .control
        .clone();
    let hub_guard = fixture.spawner.hub.lock().await;

    assert_eq!(
        super::super::control::finish_exact(
            &fixture.spawner,
            &fixture.execution,
            &control,
            AgentCompletion::new("workflow_stopped", None),
            Duration::from_millis(10),
        )
        .await,
        WorkflowCleanupStatus::TimedOut
    );
    drop(hub_guard);
    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&fixture.execution, Duration::from_secs(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
}

#[tokio::test(start_paused = true)]
async fn timed_out_finish_uses_generation_bound_fallback_when_hub_recovers() {
    let fixture = harness().await;
    let control = fixture.spawner.attempts.lock().await.by_attempt[&fixture.causation.attempt_id]
        .control
        .clone();
    let hub_guard = fixture.spawner.hub.lock().await;
    let timeout = Duration::from_millis(10);
    let finish = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move {
            super::super::control::finish_exact(
                &spawner,
                &execution,
                &control,
                AgentCompletion::new("workflow_stopped", None),
                timeout,
            )
            .await
        }
    });
    tokio::task::yield_now().await;
    tokio::time::advance(timeout).await;
    tokio::task::yield_now().await;
    assert!(!finish.is_finished());
    drop(hub_guard);

    assert_eq!(finish.await.unwrap(), WorkflowCleanupStatus::Confirmed);
    assert!(
        fixture
            .spawner
            .hub
            .lock()
            .await
            .registry
            .current_execution(&fixture.execution.address.agent)
            .is_none()
    );
    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&fixture.execution, Duration::from_secs(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
}
