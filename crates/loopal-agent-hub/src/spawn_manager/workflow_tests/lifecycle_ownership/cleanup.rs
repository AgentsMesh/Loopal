#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activation_finish_marks_stopping_when_lease_changes_after_owner_commit() {
    let fixture = harness().await;
    fixture
        .spawner
        .attempts
        .lock()
        .await
        .by_attempt
        .get_mut(&fixture.causation.attempt_id)
        .unwrap()
        .phase = super::super::AttemptPhase::Activating;
    let hub_guard = fixture.spawner.hub.lock().await;
    let reached_hub_wait = Arc::new(Notify::new());
    let reached = reached_hub_wait.notified();
    tokio::pin!(reached);
    reached.as_mut().enable();
    let activation = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        let reached_hub_wait = reached_hub_wait.clone();
        async move {
            super::super::control::finish_activation_for_test(
                &spawner,
                &execution,
                reached_hub_wait,
            )
            .await
        }
    });
    reached.await;
    let attempts_guard = fixture.spawner.attempts.lock().await;
    drop(hub_guard);
    tokio::task::yield_now().await;
    {
        let mut hub = fixture.spawner.hub.lock().await;
        assert!(hub.registry.unregister_exact(&fixture.execution));
    }
    drop(attempts_guard);

    assert!(matches!(
        activation.await.unwrap(),
        Err(WorkflowActivationFailure::Uncertain(_))
    ));
    assert!(
        fixture.spawner.attempts.lock().await.by_attempt[&fixture.causation.attempt_id].phase
            == super::super::AttemptPhase::Stopping
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_wait_before_attempt_lock_still_establishes_cleanup_owner() {
    let fixture = harness().await;
    let attempts_guard = fixture.spawner.attempts.lock().await;
    let waiter = tokio::spawn(super::super::control::shutdown(
        &fixture.spawner,
        &fixture.execution,
        Duration::from_secs(1),
    ));
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert!(!fixture.probe.process_stopped.load(Ordering::SeqCst));
    drop(attempts_guard);

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let owners = fixture.spawner.attempts.lock().await;
            if fixture.probe.process_stopped.load(Ordering::SeqCst)
                && owners.by_attempt.is_empty()
                && owners.by_execution.is_empty()
            {
                return;
            }
            drop(owners);
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test(start_paused = true)]
async fn timed_out_supervisor_retains_owner_until_exact_detach_is_confirmed() {
    let fixture = harness().await;
    let hub_guard = fixture.spawner.hub.lock().await;
    let stage_timeout = Duration::from_millis(10);

    let cleanup = super::super::control::shutdown_supervisor_for_test(
        &fixture.spawner,
        &fixture.execution,
        stage_timeout,
    );
    tokio::task::yield_now().await;
    tokio::time::advance(stage_timeout).await;
    super::support::wait_for(&fixture.probe.shutdowns, 1).await;
    while !fixture.probe.process_stopped.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(stage_timeout).await;
    tokio::task::yield_now().await;
    tokio::time::advance(stage_timeout).await;
    assert_eq!(cleanup.await.unwrap(), WorkflowCleanupStatus::TimedOut);

    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_execution.contains_key(&fixture.execution));
    drop(hub_guard);
    drop(owners);

    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&fixture.execution, Duration::from_secs(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_execution.is_empty());
}
