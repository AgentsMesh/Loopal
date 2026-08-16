use super::*;

#[tokio::test]
async fn preparation_covers_guard_timeout_failure_and_delivery_finalization_paths() {
    let owner = owner();
    let (run, key) = dispatching_run("wrun_prepare_guards", "watt_prepare_guards");
    let spawner = TestSpawner::confirmed();
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        20,
    );
    let failure = spawn_failure(WorkflowFailureClass::Permanent, "prepare timeout");
    callbacks::preparation_timed_out(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        failure.clone(),
    );
    callbacks::prepared(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        WorkflowPreparedDelivery::new(Err(failure.clone()), spawner.clone()),
    )
    .await
    .unwrap();
    callbacks::preparation_delivery_finished(&mut coordinator, owner.clone(), key.clone())
        .await
        .unwrap();

    let mut wrong_pending = pending(&owner, &key);
    wrong_pending.owner = WorkflowOwner::new("other", QualifiedAddress::local("root"));
    coordinator
        .pending
        .insert(key.attempt_id.clone(), wrong_pending);
    callbacks::preparation_timed_out(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        failure.clone(),
    );
    callbacks::prepared(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        WorkflowPreparedDelivery::new(Err(failure.clone()), spawner.clone()),
    )
    .await
    .unwrap();
    callbacks::preparation_delivery_finished(&mut coordinator, owner.clone(), key.clone())
        .await
        .unwrap();

    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    callbacks::preparation_timed_out(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        failure.clone(),
    );
    assert!(coordinator.pending[&key.attempt_id].abort_requested);
    let abort_waiter = coordinator
        .pending
        .get_mut(&key.attempt_id)
        .unwrap()
        .abort_waiter
        .take()
        .unwrap();
    assert_eq!(
        abort_waiter.await.unwrap(),
        WorkflowCleanupStatus::Confirmed
    );

    let (run, key) = dispatching_run("wrun_prepare_failure", "watt_prepare_failure");
    let journal = Arc::new(MemoryJournal::default());
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        journal.clone(),
        spawner.clone(),
        30,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    callbacks::prepared(
        &mut coordinator,
        owner.clone(),
        key,
        WorkflowPreparedDelivery::new(Err(failure.clone()), spawner.clone()),
    )
    .await
    .unwrap();
    assert!(matches!(
        journal.payloads().last(),
        Some(WorkflowEventPayload::AttemptFailed { .. })
    ));

    let run = validated_run("wrun_prepare_stale_run");
    let key = AttemptKey {
        run_id: run.id.clone(),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new("watt_prepare_stale_run"),
    };
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        40,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    let (worker, _outcome) = prepared_worker(AgentExecutionRef::local("stale", 8));
    callbacks::prepared(
        &mut coordinator,
        owner.clone(),
        key,
        WorkflowPreparedDelivery::new(Ok(worker), spawner.clone()),
    )
    .await
    .unwrap();

    let (run, key) = dispatching_run("wrun_prepare_late", "watt_prepare_late");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        50,
    );
    let mut late = pending(&owner, &key);
    late.late_execution = Some(AgentExecutionRef::local("late", 9));
    coordinator.pending.insert(key.attempt_id.clone(), late);
    callbacks::prepared(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        WorkflowPreparedDelivery::new(Err(failure), spawner),
    )
    .await
    .unwrap();

    coordinator
        .pending
        .get_mut(&key.attempt_id)
        .unwrap()
        .late_execution = None;
    coordinator
        .pending
        .get_mut(&key.attempt_id)
        .unwrap()
        .abort_status = Some(WorkflowCleanupStatus::Confirmed);
    callbacks::preparation_delivery_finished(&mut coordinator, owner, key)
        .await
        .unwrap();
    tokio::task::yield_now().await;
    assert!(matches!(
        coordinator.commands.try_recv(),
        Ok(WorkflowCommand::FinalizePreparationAbort { .. })
    ));
}
