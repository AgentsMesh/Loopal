use super::*;

#[tokio::test]
async fn stop_wrappers_cover_deadline_mismatched_execution_and_quarantine() {
    let owner = owner();
    let run = running_ready_run("wrun_stop_deadline");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        20,
    );
    stop::expire_run_deadline(&mut coordinator, owner.clone(), run, 2_000)
        .await
        .unwrap();
    assert_eq!(
        coordinator
            .state
            .owned_snapshot(&owner, &WorkflowRunId::new("wrun_stop_deadline"))
            .unwrap()
            .state,
        WorkflowRunState::Failed
    );

    let (run, key) = running_attempt_run("wrun_stop_mismatch", "watt_stop_mismatch");
    let (mut coordinator, _commands) = self::coordinator(
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
            AgentExecutionRef::local("worker", 16),
            ActiveAttemptPhase::Running,
        ),
    );
    stop::request_failure_stop(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        AgentExecutionRef::local("worker", 17),
        spawn_failure(WorkflowFailureClass::Permanent, "wrong generation"),
        "mismatched execution",
        31,
    )
    .await
    .unwrap();
    assert!(coordinator.active[&key.attempt_id].stop.is_none());

    let (run, key) = running_attempt_run("wrun_stop_quarantine", "watt_stop_quarantine");
    let spawner = TestSpawner::confirmed();
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        40,
    );
    coordinator.active.insert(
        key.attempt_id.clone(),
        active(
            &owner,
            &key,
            AgentExecutionRef::local("quarantine", 18),
            ActiveAttemptPhase::Running,
        ),
    );
    stop::quarantine_owner(&mut coordinator, &owner);
    assert!(coordinator.active.is_empty());
    tokio::task::yield_now().await;
    assert_eq!(spawner.shutdowns.load(Ordering::SeqCst), 1);
}
