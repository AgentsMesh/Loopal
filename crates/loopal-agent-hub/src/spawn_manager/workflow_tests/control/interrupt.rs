#[tokio::test]
async fn stale_generation_controls_have_no_side_effects() {
    let fixture = harness().await;
    let stale = AgentExecutionRef::local(
        fixture.execution.address.agent.clone(),
        fixture.execution.connection_generation + 1,
    );

    assert!(matches!(
        fixture.spawner.activate(&stale).await,
        Err(WorkflowActivationFailure::Stopped(_))
    ));
    assert_eq!(
        fixture.spawner.interrupt(&stale).await,
        WorkflowStopStatus::Stopped
    );
    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&stale, Duration::from_millis(50))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
    assert_eq!(fixture.probe.starts.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.probe.interrupts.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.probe.shutdowns.load(Ordering::SeqCst), 0);
    assert!(!fixture.probe.process_stopped.load(Ordering::SeqCst));
    let owners = fixture.spawner.attempts.lock().await;
    assert!(owners.by_execution.contains_key(&fixture.execution));
    assert!(
        owners
            .by_attempt
            .contains_key(&fixture.causation.attempt_id)
    );
}

#[tokio::test]
async fn missing_registry_interrupt_keeps_existing_workflow_custody_requested() {
    let fixture = harness().await;
    {
        let mut hub = fixture.spawner.hub.lock().await;
        assert!(hub.registry.unregister_exact(&fixture.execution));
    }

    // The workflow attempt owner still exists even though the registry
    // operation disappeared. Treating this as `Stopped` would make the
    // coordinator discard custody without ever running shutdown cleanup.
    assert_eq!(
        fixture.spawner.interrupt(&fixture.execution).await,
        WorkflowStopStatus::Requested
    );
    assert_eq!(fixture.probe.interrupts.load(Ordering::SeqCst), 0);
    assert!(
        fixture
            .spawner
            .attempts
            .lock()
            .await
            .by_attempt
            .contains_key(&fixture.causation.attempt_id)
    );
}

#[tokio::test]
async fn interrupt_rechecks_exact_owner_after_waiting_for_the_operation_lock() {
    let fixture = harness().await;
    let operation = fixture.spawner.attempts.lock().await.by_attempt[&fixture.causation.attempt_id]
        .operation
        .clone();
    let operation_guard = operation.lock().await;
    let interruption = tokio::spawn({
        let spawner = fixture.spawner.clone();
        let execution = fixture.execution.clone();
        async move { spawner.interrupt(&execution).await }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while Arc::strong_count(&operation) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    {
        let mut owners = fixture.spawner.attempts.lock().await;
        assert!(super::super::remove_exact_owner(
            &mut owners,
            &fixture.execution
        ));
    }
    drop(operation_guard);

    assert_eq!(interruption.await.unwrap(), WorkflowStopStatus::Stopped);
    assert_eq!(fixture.probe.interrupts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_interrupt_transport_retains_cleanup_custody() {
    let fixture = harness().await;
    let connection = fixture.spawner.attempts.lock().await.by_attempt
        [&fixture.causation.attempt_id]
        .control
        .connection
        .clone();
    connection.close().await;

    assert_eq!(
        fixture.spawner.interrupt(&fixture.execution).await,
        WorkflowStopStatus::Requested
    );
    assert!(
        fixture
            .spawner
            .attempts
            .lock()
            .await
            .by_execution
            .contains_key(&fixture.execution)
    );
    assert_eq!(
        fixture
            .spawner
            .shutdown_and_wait(&fixture.execution, Duration::from_secs(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
}
