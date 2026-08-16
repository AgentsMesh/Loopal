use super::*;

#[tokio::test]
async fn drain_covers_terminal_guards_payloads_cleanup_sources_and_failures() {
    let owner = owner();

    let key = AttemptKey {
        run_id: WorkflowRunId::new("wrun_drain_missing"),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new("watt_drain_missing"),
    };
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        Vec::new(),
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        20,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    drainage::run(&mut coordinator).await.unwrap();

    let (run, key) = terminal_run("wrun_drain_terminal", "watt_drain_terminal");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        30,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    drainage::run(&mut coordinator).await.unwrap();

    let (run, key) = cancelling_dispatch_run("wrun_drain_cancel", "watt_drain_cancel");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        40,
    );
    let mut attempt = pending(&owner, &key);
    attempt.stop = Some(StopDisposition::Cancelled("drain cancellation".into()));
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    drainage::run(&mut coordinator).await.unwrap();

    let (run, key) = dispatching_run("wrun_drain_failed", "watt_drain_failed");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        50,
    );
    let mut attempt = pending(&owner, &key);
    attempt.stop = Some(StopDisposition::Failed(spawn_failure(
        WorkflowFailureClass::TransientBeforeExecution,
        "drain failure",
    )));
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    drainage::run(&mut coordinator).await.unwrap();

    let (run, key) = dispatching_run("wrun_drain_late", "watt_drain_late");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        60,
    );
    let mut attempt = pending(&owner, &key);
    attempt.late_execution = Some(AgentExecutionRef::local("late", 12));
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    drainage::run(&mut coordinator).await.unwrap();

    let (run, key) = dispatching_run("wrun_drain_waiter", "watt_drain_waiter");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        70,
    );
    let mut attempt = pending(&owner, &key);
    attempt.abort_waiter = Some(tokio::spawn(async { WorkflowCleanupStatus::Confirmed }));
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    drainage::run(&mut coordinator).await.unwrap();

    let (run, key) = running_attempt_run("wrun_drain_active", "watt_drain_active");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        80,
    );
    coordinator.active.insert(
        key.attempt_id.clone(),
        active(
            &owner,
            &key,
            AgentExecutionRef::local("active", 13),
            ActiveAttemptPhase::Running,
        ),
    );
    drainage::run(&mut coordinator).await.unwrap();

    for (run_id, attempt_id, active_attempt) in [
        (
            "wrun_drain_pending_timeout",
            "watt_drain_pending_timeout",
            false,
        ),
        (
            "wrun_drain_active_timeout",
            "watt_drain_active_timeout",
            true,
        ),
    ] {
        let (run, key) = if active_attempt {
            running_attempt_run(run_id, attempt_id)
        } else {
            dispatching_run(run_id, attempt_id)
        };
        let (mut coordinator, _commands) = self::coordinator(
            WorkflowCoordinatorMode::ExecutionHarness,
            true,
            vec![run],
            Arc::new(MemoryJournal::default()),
            TestSpawner::timed_out(),
            90,
        );
        if active_attempt {
            coordinator.active.insert(
                key.attempt_id.clone(),
                active(
                    &owner,
                    &key,
                    AgentExecutionRef::local("timeout", 14),
                    ActiveAttemptPhase::Running,
                ),
            );
        } else {
            coordinator
                .pending
                .insert(key.attempt_id.clone(), pending(&owner, &key));
        }
        assert_eq!(
            drainage::run(&mut coordinator).await,
            Err(WorkflowCoordinatorError::CleanupTimeout)
        );
    }

    let (run, key) = dispatching_run("wrun_drain_panic", "watt_drain_panic");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::panicking_abort(),
        100,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    assert_eq!(
        drainage::run(&mut coordinator).await,
        Err(WorkflowCoordinatorError::CleanupTimeout)
    );

    let (run, key) = dispatching_run("wrun_drain_journal", "watt_drain_journal");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        MemoryJournal::failing(),
        TestSpawner::confirmed(),
        110,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    assert_eq!(
        drainage::run(&mut coordinator).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
}
