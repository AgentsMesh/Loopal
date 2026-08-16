use super::*;

#[tokio::test]
async fn run_deadline_ignores_unrelated_attempts_and_preserves_an_existing_stop() {
    let owner = owner();
    let run = running_ready_run("wrun_deadline_target");
    let target_id = run.id.clone();
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        20,
    );
    let unrelated = AttemptKey {
        run_id: WorkflowRunId::new("wrun_deadline_other"),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new("watt_deadline_other"),
    };
    coordinator
        .pending
        .insert(unrelated.attempt_id.clone(), pending(&owner, &unrelated));
    coordinator.active.insert(
        WorkflowAttemptId::new("watt_deadline_active_other"),
        active(
            &owner,
            &AttemptKey {
                attempt_id: WorkflowAttemptId::new("watt_deadline_active_other"),
                ..unrelated.clone()
            },
            AgentExecutionRef::local("other", 1),
            ActiveAttemptPhase::Running,
        ),
    );
    stop::expire_run_deadline(&mut coordinator, owner.clone(), run, 2_000)
        .await
        .unwrap();
    assert_eq!(
        coordinator
            .state
            .owned_snapshot(&owner, &target_id)
            .unwrap()
            .state,
        WorkflowRunState::Failed
    );

    let (run, key) = dispatching_run("wrun_deadline_stopped", "watt_deadline_stopped");
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run.clone()],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        30,
    );
    let mut stopped = pending(&owner, &key);
    stopped.stop = Some(StopDisposition::Cancelled("already stopping".into()));
    coordinator.pending.insert(key.attempt_id.clone(), stopped);
    stop::expire_run_deadline(&mut coordinator, owner.clone(), run, 2_000)
        .await
        .unwrap();
    let waiter = coordinator
        .pending
        .get_mut(&key.attempt_id)
        .unwrap()
        .abort_waiter
        .take()
        .unwrap();
    assert_eq!(waiter.await.unwrap(), WorkflowCleanupStatus::Confirmed);
}

#[tokio::test]
async fn pending_abort_rejects_unrelated_duplicate_and_stale_attempts() {
    let owner = owner();
    let (run, key) = dispatching_run("wrun_abort_edges", "watt_abort_edges");
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        40,
    );
    let unrelated = AttemptKey {
        run_id: WorkflowRunId::new("wrun_abort_other"),
        node_id: key.node_id.clone(),
        attempt_id: WorkflowAttemptId::new("watt_abort_other"),
    };
    coordinator
        .pending
        .insert(unrelated.attempt_id.clone(), pending(&owner, &unrelated));
    stop::begin_cancel_effects(
        &mut coordinator,
        owner.clone(),
        key.run_id.clone(),
        "ignore unrelated".into(),
    );

    let mut duplicate = pending(&owner, &key);
    duplicate.abort_requested = true;
    coordinator.pending.clear();
    coordinator
        .pending
        .insert(key.attempt_id.clone(), duplicate);
    stop::begin_cancel_effects(
        &mut coordinator,
        owner.clone(),
        key.run_id.clone(),
        "duplicate abort".into(),
    );
    coordinator.pending.clear();
    stop::request_pending_attempt_abort(&mut coordinator, &owner, &key);

    let mut wrong_owner = pending(&owner, &key);
    wrong_owner.owner = WorkflowOwner::new("other", QualifiedAddress::local("root"));
    coordinator
        .pending
        .insert(key.attempt_id.clone(), wrong_owner);
    stop::request_pending_attempt_abort(&mut coordinator, &owner, &key);

    let mut wrong_key = pending(&owner, &key);
    wrong_key.key.node_id = WorkflowNodeId::new("other");
    coordinator
        .pending
        .insert(key.attempt_id.clone(), wrong_key);
    stop::request_pending_attempt_abort(&mut coordinator, &owner, &key);

    let mut duplicate = pending(&owner, &key);
    duplicate.abort_requested = true;
    coordinator
        .pending
        .insert(key.attempt_id.clone(), duplicate);
    stop::request_pending_attempt_abort(&mut coordinator, &owner, &key);
    assert!(coordinator.pending[&key.attempt_id].abort_waiter.is_none());
}
