use super::*;

#[tokio::test]
async fn pending_stop_covers_unprepared_bound_duplicate_timeout_and_poison_paths() {
    let owner = owner();
    let spawner = TestSpawner::confirmed();

    let (run, key) = cancelling_dispatch_run("wrun_pending_unprepared", "watt_pending_unprepared");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        20,
    );
    stop::finish_preparation_stop(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        Err(spawn_failure(
            WorkflowFailureClass::Permanent,
            "preparation stopped",
        )),
        run,
        pending(&owner, &key),
    )
    .await
    .unwrap();
    assert_eq!(
        coordinator
            .state
            .owned_snapshot(&owner, &key.run_id)
            .unwrap()
            .state,
        WorkflowRunState::Cancelled
    );

    let (run, key) = cancelling_dispatch_run("wrun_pending_bound", "watt_pending_bound");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        30,
    );
    let (worker, _outcome) = prepared_worker(AgentExecutionRef::local("bound-worker", 10));
    stop::finish_preparation_stop(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        Ok(worker),
        run,
        pending(&owner, &key),
    )
    .await
    .unwrap();
    assert_eq!(
        coordinator.active[&key.attempt_id].phase,
        ActiveAttemptPhase::Interrupting
    );

    let (run, key) = cancelling_dispatch_run("wrun_pending_duplicate", "watt_pending_duplicate");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        40,
    );
    let execution = AgentExecutionRef::local("duplicate-worker", 11);
    let existing_key = AttemptKey {
        run_id: WorkflowRunId::new("wrun_existing"),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new("watt_existing"),
    };
    coordinator.active.insert(
        existing_key.attempt_id.clone(),
        active(
            &owner,
            &existing_key,
            execution.clone(),
            ActiveAttemptPhase::Running,
        ),
    );
    let (worker, _outcome) = prepared_worker(execution);
    assert_eq!(
        stop::finish_preparation_stop(
            &mut coordinator,
            owner.clone(),
            key.clone(),
            Ok(worker),
            run,
            pending(&owner, &key),
        )
        .await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );
    assert!(coordinator.state.is_poisoned(&owner));

    let (run, key) = dispatching_run("wrun_pending_timeout", "watt_pending_timeout");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        50,
    );
    stop::pending::terminalize_after_abort(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        run,
        pending(&owner, &key),
        WorkflowCleanupStatus::TimedOut,
    )
    .await
    .unwrap();

    let (run, key) = dispatching_run("wrun_pending_poison", "watt_pending_poison");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        spawner,
        60,
    );
    coordinator.state.poison(owner.clone());
    stop::pending::terminalize_after_abort(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        run,
        pending(&owner, &key),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();
}
