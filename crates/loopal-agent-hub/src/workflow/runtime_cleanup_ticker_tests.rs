use std::sync::Arc;

use tokio::sync::oneshot;

use super::super::cleanup::{DropCleanupOutcome, RuntimeCleanup};
use super::super::ticker;
use super::super::*;
use crate::workflow::WorkflowCoordinator;

#[tokio::test]
async fn ticker_stops_idempotently_and_joins_cleanly() {
    let hub = super::test_hub();
    let (handle, actor) = WorkflowCoordinator::spawn_disabled();
    let mut ticker = Some(ticker::start_ticker(hub, handle.clone()));

    ticker::request_stop(&mut ticker);
    ticker::request_stop(&mut ticker);
    assert!(ticker::join(&mut ticker).await.is_ok());
    assert!(ticker.is_none());

    handle.shutdown().await.unwrap();
    actor.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn ticker_failure_clears_its_exact_admission_and_requests_shutdown() {
    let hub = super::test_hub();
    let (commands, receiver) = tokio::sync::mpsc::channel(1);
    drop(receiver);
    let handle = WorkflowCoordinatorHandle { commands };
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());
    let shutdown_signal = hub.lock().await.shutdown_signal.clone();
    let mut ticker = Some(ticker::start_ticker(hub.clone(), handle));

    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_millis(250)).await;
    assert!(matches!(
        ticker::join(&mut ticker).await,
        Err(WorkflowRuntimeError::Tick(
            WorkflowCoordinatorError::Unavailable
        ))
    ));
    shutdown_signal.notified().await;
    assert!(hub.lock().await.workflow_coordinator().is_none());
}

#[tokio::test]
async fn cleanup_returns_actor_join_failure_after_stopping_the_coordinator() {
    let hub = super::test_hub();
    let shutdown_signal = hub.lock().await.shutdown_signal.clone();
    let (handle, coordinator) = WorkflowCoordinator::spawn_disabled();
    let actor = tokio::spawn(async { panic!("actor failure") });
    let cleanup = RuntimeCleanup::new(hub, shutdown_signal, handle, Some(actor), None, false);

    let error = cleanup.shutdown().await.unwrap_err();
    assert!(matches!(
        error,
        WorkflowRuntimeError::TaskJoin {
            task: "workflow coordinator",
            ..
        }
    ));
    coordinator.await.unwrap();
}

#[tokio::test]
async fn cleanup_supervisor_escalates_after_a_graceful_failure() {
    let hub = super::test_hub();
    let shutdown_signal = hub.lock().await.shutdown_signal.clone();
    let (handle, coordinator) = WorkflowCoordinator::spawn_disabled();
    let actor = tokio::spawn(async { panic!("actor failure") });
    let (probe, outcome) = oneshot::channel();
    let cleanup = RuntimeCleanup::new(
        Arc::clone(&hub),
        shutdown_signal.clone(),
        handle,
        Some(actor),
        None,
        false,
    );

    cleanup.spawn_supervisor(std::time::Duration::from_secs(1), Some(probe));
    assert_eq!(outcome.await.unwrap(), DropCleanupOutcome::Escalated);
    shutdown_signal.notified().await;
    coordinator.await.unwrap();
}

#[tokio::test]
async fn dropping_unsettled_cleanup_aborts_actor_and_clears_exact_admission() {
    let hub = super::test_hub();
    let shutdown_signal = hub.lock().await.shutdown_signal.clone();
    let (handle, actor) = WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());
    let cleanup = RuntimeCleanup::new(
        hub.clone(),
        shutdown_signal.clone(),
        handle.clone(),
        Some(actor),
        None,
        true,
    );
    let shutdown = shutdown_signal.notified();

    drop(cleanup);
    shutdown.await;
    tokio::task::yield_now().await;

    assert!(hub.lock().await.workflow_coordinator().is_none());
    assert_eq!(
        handle.shutdown().await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
}

#[tokio::test]
async fn contended_escalation_requests_shutdown_without_clearing_an_unlocked_hub_later() {
    let hub = super::test_hub();
    let shutdown_signal = hub.lock().await.shutdown_signal.clone();
    let (handle, actor) = WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());
    let cleanup = RuntimeCleanup::new(
        hub.clone(),
        shutdown_signal.clone(),
        handle.clone(),
        Some(actor),
        None,
        true,
    );
    let mut guard = hub.lock().await;
    let shutdown = shutdown_signal.notified();

    drop(cleanup);
    assert!(
        guard
            .workflow_coordinator()
            .is_some_and(|current| current.same_channel(&handle))
    );
    guard.clear_workflow_coordinator();
    drop(guard);
    shutdown.await;
}

#[tokio::test]
async fn abort_stops_and_consumes_a_live_ticker() {
    let hub = super::test_hub();
    let (handle, actor) = WorkflowCoordinator::spawn_disabled();
    let mut ticker = Some(ticker::start_ticker(hub, handle.clone()));

    ticker::abort(&mut ticker);
    assert!(ticker.is_none());

    handle.shutdown().await.unwrap();
    actor.await.unwrap();
}
