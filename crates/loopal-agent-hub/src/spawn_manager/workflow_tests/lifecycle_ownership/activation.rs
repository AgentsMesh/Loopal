#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activation_completion_releases_attempts_before_waiting_for_hub() {
    let fixture = harness().await;
    {
        let mut owners = fixture.spawner.attempts.lock().await;
        owners
            .by_attempt
            .get_mut(&fixture.causation.attempt_id)
            .expect("harness owner")
            .phase = super::super::AttemptPhase::Activating;
    }

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

    let attempts_lock =
        tokio::time::timeout(Duration::from_secs(1), fixture.spawner.attempts.lock())
            .await
            .expect("activation must release attempts before waiting for Hub");
    assert!(matches!(
        attempts_lock.by_attempt[&fixture.causation.attempt_id].phase,
        super::super::AttemptPhase::Activating
    ));
    drop(attempts_lock);
    drop(hub_guard);

    assert!(activation.await.unwrap().is_ok());
}

#[tokio::test]
async fn activation_finish_rejects_missing_and_nonactivating_exact_owners() {
    let fixture = harness().await;
    let stale = AgentExecutionRef::local(
        fixture.execution.address.agent.clone(),
        fixture.execution.connection_generation + 1,
    );
    assert!(matches!(
        super::super::control::finish_activation_for_test(
            &fixture.spawner,
            &stale,
            Arc::new(Notify::new()),
        )
        .await,
        Err(WorkflowActivationFailure::Uncertain(_))
    ));
    assert!(matches!(
        super::super::control::finish_activation_for_test(
            &fixture.spawner,
            &fixture.execution,
            Arc::new(Notify::new()),
        )
        .await,
        Err(WorkflowActivationFailure::Uncertain(_))
    ));
}

#[tokio::test]
async fn activation_finish_detects_a_lease_lost_before_its_first_check() {
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
    assert!(
        fixture
            .spawner
            .hub
            .lock()
            .await
            .registry
            .unregister_exact(&fixture.execution)
    );

    assert!(matches!(
        super::super::control::finish_activation_for_test(
            &fixture.spawner,
            &fixture.execution,
            Arc::new(Notify::new()),
        )
        .await,
        Err(WorkflowActivationFailure::Uncertain(_))
    ));
}
