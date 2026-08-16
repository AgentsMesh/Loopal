use super::*;

#[tokio::test]
async fn preparation_guards_require_the_exact_key_and_preserve_an_existing_stop() {
    let owner = owner();
    let (run, key) = dispatching_run("wrun_prepare_key", "watt_prepare_key");
    let spawner = TestSpawner::confirmed();
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        20,
    );
    let failure = spawn_failure(WorkflowFailureClass::Permanent, "preparation edge");
    let mut mismatched = pending(&owner, &key);
    mismatched.key.node_id = WorkflowNodeId::new("other");
    coordinator
        .pending
        .insert(key.attempt_id.clone(), mismatched);

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
    assert!(!coordinator.pending[&key.attempt_id].delivery_finished);

    let mut stopped = pending(&owner, &key);
    stopped.stop = Some(StopDisposition::Cancelled("already stopped".into()));
    coordinator.pending.insert(key.attempt_id.clone(), stopped);
    callbacks::preparation_timed_out(&mut coordinator, owner, key.clone(), failure);
    assert!(matches!(
        coordinator.pending[&key.attempt_id].stop,
        Some(StopDisposition::Cancelled(_))
    ));
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
async fn prepared_routes_cancelling_stopped_and_stale_running_snapshots() {
    let owner = owner();
    let spawner = TestSpawner::confirmed();
    let failure = spawn_failure(WorkflowFailureClass::Permanent, "preparation edge");

    let (run, key) = cancelling_dispatch_run("wrun_prepare_cancel", "watt_prepare_cancel");
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        30,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    callbacks::prepared(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        WorkflowPreparedDelivery::new(Err(failure.clone()), spawner.clone()),
    )
    .await
    .unwrap();

    let (run, key) = dispatching_run("wrun_prepare_stop", "watt_prepare_stop");
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        40,
    );
    let mut stopped = pending(&owner, &key);
    stopped.stop = Some(StopDisposition::Failed(failure.clone()));
    coordinator.pending.insert(key.attempt_id.clone(), stopped);
    callbacks::prepared(
        &mut coordinator,
        owner.clone(),
        key,
        WorkflowPreparedDelivery::new(Err(failure.clone()), spawner.clone()),
    )
    .await
    .unwrap();

    let run = running_ready_run("wrun_prepare_missing_attempt");
    let key = AttemptKey {
        run_id: run.id.clone(),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new("watt_prepare_missing_attempt"),
    };
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        50,
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

    let (run, key) = running_attempt_run("wrun_prepare_wrong_state", "watt_prepare_wrong_state");
    let (mut coordinator, _commands) = super::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner.clone(),
        60,
    );
    coordinator
        .pending
        .insert(key.attempt_id.clone(), pending(&owner, &key));
    callbacks::prepared(
        &mut coordinator,
        owner,
        key,
        WorkflowPreparedDelivery::new(Err(failure), spawner),
    )
    .await
    .unwrap();
}
