use super::*;

#[tokio::test]
async fn stop_effects_handle_detached_callbacks_and_non_interrupting_attempts() {
    let owner = owner();
    let (run, key) = running_attempt_run("wrun_effect_detached", "watt_effect_detached");
    let (mut coordinator, commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        20,
    );
    coordinator.active.insert(
        key.attempt_id.clone(),
        active(
            &owner,
            &key,
            AgentExecutionRef::local("worker", 1),
            ActiveAttemptPhase::Running,
        ),
    );
    drop(commands);
    stop::begin_cancel_effects(
        &mut coordinator,
        owner.clone(),
        key.run_id.clone(),
        "detached interrupt".into(),
    );
    tokio::task::yield_now().await;
    assert_eq!(
        coordinator.active[&key.attempt_id].phase,
        ActiveAttemptPhase::Interrupting
    );

    let (run, key) = running_attempt_run("wrun_effect_running", "watt_effect_running");
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        30,
    );
    coordinator.active.insert(
        key.attempt_id.clone(),
        active(
            &owner,
            &key,
            AgentExecutionRef::local("worker", 2),
            ActiveAttemptPhase::Running,
        ),
    );
    stop::tick(&mut coordinator, 50).await.unwrap();
    assert_eq!(
        coordinator.active[&key.attempt_id].phase,
        ActiveAttemptPhase::Running
    );

    let (run, key) = running_attempt_run("wrun_effect_shutdown", "watt_effect_shutdown");
    let (mut coordinator, commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        40,
    );
    let mut stopping = active(
        &owner,
        &key,
        AgentExecutionRef::local("worker", 3),
        ActiveAttemptPhase::Interrupting,
    );
    stopping.shutdown_after_unix_ms = Some(1);
    coordinator.active.insert(key.attempt_id.clone(), stopping);
    drop(commands);
    stop::tick(&mut coordinator, 50).await.unwrap();
    let waiter = coordinator
        .active
        .get_mut(&key.attempt_id)
        .unwrap()
        .shutdown_waiter
        .take()
        .unwrap();
    assert_eq!(waiter.await.unwrap(), WorkflowCleanupStatus::Confirmed);
}

#[tokio::test]
async fn stopped_callback_rejects_every_stale_execution_identity() {
    let owner = owner();
    let (run, key) = running_attempt_run("wrun_effect_guards", "watt_effect_guards");
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        50,
    );
    let execution = AgentExecutionRef::local("worker", 4);
    stop::stopped(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        execution.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();

    coordinator.active.insert(
        key.attempt_id.clone(),
        active(
            &owner,
            &key,
            execution.clone(),
            ActiveAttemptPhase::ShuttingDown,
        ),
    );
    let wrong_owner = WorkflowOwner::new("other", QualifiedAddress::local("root"));
    stop::stopped(
        &mut coordinator,
        wrong_owner,
        key.clone(),
        execution.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();
    let mut wrong_key = key.clone();
    wrong_key.node_id = WorkflowNodeId::new("other");
    stop::stopped(
        &mut coordinator,
        owner.clone(),
        wrong_key,
        execution.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();
    stop::stopped(
        &mut coordinator,
        owner,
        key.clone(),
        AgentExecutionRef::local("worker", 5),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();
    assert!(coordinator.active.contains_key(&key.attempt_id));
}
