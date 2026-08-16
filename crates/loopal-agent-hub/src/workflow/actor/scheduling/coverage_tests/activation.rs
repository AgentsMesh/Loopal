use super::*;

#[tokio::test]
async fn activation_covers_stale_stopped_uncertain_and_detached_outcomes() {
    let owner = owner();
    let journal = Arc::new(MemoryJournal::default());
    let spawner = TestSpawner::confirmed();
    let (run, key) = bound_run("wrun_activation_stopped", "watt_activation_stopped");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        journal.clone(),
        spawner.clone(),
        20,
    );
    let execution = AgentExecutionRef::local("worker", 1);
    coordinator.active.insert(
        key.attempt_id.clone(),
        active(
            &owner,
            &key,
            execution.clone(),
            ActiveAttemptPhase::Activating,
        ),
    );

    callbacks::activated(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        AgentExecutionRef::local("worker", 2),
        Ok(()),
    )
    .await
    .unwrap();
    callbacks::activated(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        execution,
        Err(WorkflowActivationFailure::Stopped(spawn_failure(
            WorkflowFailureClass::Permanent,
            "activation stopped",
        ))),
    )
    .await
    .unwrap();
    assert!(coordinator.active.is_empty());
    assert!(matches!(
        journal.payloads().last(),
        Some(WorkflowEventPayload::AttemptFailed { .. })
    ));

    let (run, key) = bound_run("wrun_activation_uncertain", "watt_activation_uncertain");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner,
        30,
    );
    let execution = AgentExecutionRef::local("worker", 3);
    coordinator.active.insert(
        key.attempt_id.clone(),
        active(
            &owner,
            &key,
            execution.clone(),
            ActiveAttemptPhase::Activating,
        ),
    );
    callbacks::activated(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        execution,
        Err(WorkflowActivationFailure::Uncertain(
            WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "activation reply was lost".into(),
            },
        )),
    )
    .await
    .unwrap();
    assert_eq!(
        coordinator.active[&key.attempt_id].phase,
        ActiveAttemptPhase::Interrupting
    );

    let (run, key) = running_attempt_run("wrun_outcome_detached", "watt_outcome_detached");
    let (mut coordinator, commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        40,
    );
    let execution = AgentExecutionRef::local("worker", 4);
    coordinator.active.insert(
        key.attempt_id.clone(),
        active(&owner, &key, execution.clone(), ActiveAttemptPhase::Running),
    );
    callbacks::spawn_outcome_waiter(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        execution.clone(),
    );
    let (worker, outcome) = prepared_worker(execution.clone());
    coordinator.active.get_mut(&key.attempt_id).unwrap().outcome = Some(worker.outcome);
    drop(commands);
    callbacks::spawn_outcome_waiter(&mut coordinator, owner, key.clone(), execution);
    let waiter = coordinator
        .active
        .get_mut(&key.attempt_id)
        .unwrap()
        .outcome_waiter
        .take()
        .unwrap();
    outcome
        .send(WorkflowWorkerOutcome::Failed(spawn_failure(
            WorkflowFailureClass::Permanent,
            "detached outcome",
        )))
        .unwrap();
    waiter.await.unwrap();
}
