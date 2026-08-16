#[tokio::test]
async fn activation_starts_only_the_exact_prepared_execution() {
    let fixture = harness().await;

    assert!(fixture.spawner.activate(&fixture.execution).await.is_ok());

    assert_eq!(fixture.probe.starts.load(Ordering::SeqCst), 1);
    assert!(matches!(
        fixture.spawner.activate(&fixture.execution).await,
        Err(WorkflowActivationFailure::Stopped(_))
    ));
    assert_eq!(fixture.probe.starts.load(Ordering::SeqCst), 1);
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_attempt[&fixture.causation.attempt_id].phase == AttemptPhase::Running);
    drop(owners);
    assert_eq!(
        fixture
            .spawner
            .hub
            .lock()
            .await
            .registry
            .agent_info(&fixture.execution.address.agent)
            .unwrap()
            .lifecycle,
        crate::AgentLifecycle::Running
    );
}

#[tokio::test]
async fn exact_interrupt_and_shutdown_clear_both_owner_indexes() {
    let fixture = harness().await;
    assert!(fixture.spawner.activate(&fixture.execution).await.is_ok());

    assert_eq!(
        fixture.spawner.interrupt(&fixture.execution).await,
        WorkflowStopStatus::Requested
    );
    wait_for(&fixture.probe.interrupts, 1).await;
    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&fixture.execution, Duration::from_secs(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );

    assert_eq!(fixture.probe.shutdowns.load(Ordering::SeqCst), 1);
    assert!(fixture.probe.process_stopped.load(Ordering::SeqCst));
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_attempt.is_empty());
    assert!(owners.by_execution.is_empty());
    drop(owners);
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
}

#[tokio::test]
async fn cancelled_wait_does_not_cancel_hung_shutdown_cleanup() {
    let fixture = harness().await;
    fixture
        .probe
        .reply_to_shutdown
        .store(false, Ordering::SeqCst);
    let spawner = fixture.spawner.clone();
    let execution = fixture.execution.clone();
    let waiter = tokio::spawn(async move {
        spawner
            .shutdown_and_wait(&execution, Duration::from_millis(50))
            .await
    });
    wait_for(&fixture.probe.shutdowns, 1).await;
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());

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

#[tokio::test]
async fn duplicate_attempt_is_rejected_before_spawn() {
    let fixture = harness().await;
    let duplicate = super::requests::causation(
        "wrun_duplicate",
        "wnode_duplicate",
        fixture.causation.attempt_id.as_str(),
    );

    let failure = match fixture.spawner.prepare(request(duplicate)).await {
        Ok(_) => panic!("duplicate attempt must be rejected"),
        Err(failure) => failure,
    };

    assert!(failure.failure.reason.contains("duplicate"));
    assert_eq!(fixture.probe.starts.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.spawner.attempts.lock().await.by_attempt.len(), 1);
}
