use super::*;

fn abort_case(suffix: &str) -> (WorkflowCoordinator, WorkflowOwner, AttemptKey) {
    let owner = owner();
    let (run, key) = cancelling_dispatch_run(
        &format!("wrun_abort_{suffix}"),
        &format!("watt_abort_{suffix}"),
    );
    let (mut coordinator, _commands) = coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        20,
    );
    let mut attempt = pending(&owner, &key);
    attempt.abort_requested = true;
    attempt.stop = Some(StopDisposition::Cancelled("coverage abort".into()));
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    (coordinator, owner, key)
}

#[tokio::test]
async fn preparation_aborted_covers_guard_and_finished_delivery_paths() {
    let (mut coordinator, owner, key) = abort_case("guards");
    coordinator.pending.get_mut(&key.attempt_id).unwrap().owner =
        WorkflowOwner::new("other", QualifiedAddress::local("root"));
    callbacks::preparation_aborted(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();

    let mut wrong_key = key.clone();
    wrong_key.node_id = WorkflowNodeId::new("other");
    let mut attempt = pending(&owner, &key);
    attempt.abort_requested = true;
    attempt.key = wrong_key;
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    callbacks::preparation_aborted(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();

    let mut attempt = pending(&owner, &key);
    attempt.abort_requested = false;
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    callbacks::preparation_aborted(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();

    let task = tokio::spawn(async {});
    while !task.is_finished() {
        tokio::task::yield_now().await;
    }
    let mut attempt = pending(&owner, &key);
    attempt.abort_requested = true;
    attempt.prepare_abort = Some(task);
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    callbacks::preparation_aborted(
        &mut coordinator,
        owner,
        key.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();
    assert!(!coordinator.pending[&key.attempt_id].delivery_finished);
}

#[tokio::test]
async fn abort_settlement_covers_every_stable_tombstone_guard() {
    let (mut coordinator, owner, key) = abort_case("settlement");
    coordinator.pending.remove(&key.attempt_id);
    callbacks::preparation_abort_settled(&mut coordinator, owner.clone(), key.clone())
        .await
        .unwrap();

    for guard in 0..6 {
        let mut attempt = pending(&owner, &key);
        attempt.abort_requested = true;
        attempt.delivery_finished = true;
        attempt.abort_status = Some(WorkflowCleanupStatus::Confirmed);
        match guard {
            0 => attempt.owner = WorkflowOwner::new("other", QualifiedAddress::local("root")),
            1 => attempt.key.node_id = WorkflowNodeId::new("other"),
            2 => attempt.abort_requested = false,
            3 => attempt.late_execution = Some(AgentExecutionRef::local("late", 1)),
            4 => attempt.delivery_finished = false,
            5 => attempt.abort_status = None,
            _ => unreachable!(),
        }
        coordinator.pending.insert(key.attempt_id.clone(), attempt);
        callbacks::preparation_abort_settled(&mut coordinator, owner.clone(), key.clone())
            .await
            .unwrap();
    }

    let mut attempt = pending(&owner, &key);
    attempt.abort_requested = true;
    attempt.delivery_finished = true;
    attempt.abort_status = Some(WorkflowCleanupStatus::Confirmed);
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    coordinator.state.poison(owner.clone());
    callbacks::preparation_abort_settled(&mut coordinator, owner, key.clone())
        .await
        .unwrap();
    assert!(!coordinator.pending.contains_key(&key.attempt_id));
}

#[tokio::test]
async fn late_shutdown_covers_stale_identity_and_closed_callback_paths() {
    let (mut coordinator, owner, key) = abort_case("late");
    let execution = AgentExecutionRef::local("late", 7);
    callbacks::late_preparation_shutdown(
        &mut coordinator,
        owner.clone(),
        key.clone(),
        execution.clone(),
        WorkflowCleanupStatus::Confirmed,
    )
    .await
    .unwrap();

    let mut attempt = pending(&owner, &key);
    attempt.late_execution = Some(execution.clone());
    coordinator.pending.insert(key.attempt_id.clone(), attempt);
    for (callback_owner, callback_key, callback_execution) in [
        (
            WorkflowOwner::new("other", QualifiedAddress::local("root")),
            key.clone(),
            execution.clone(),
        ),
        (
            { owner.clone() },
            {
                let mut wrong = key.clone();
                wrong.node_id = WorkflowNodeId::new("other");
                wrong
            },
            execution.clone(),
        ),
        (
            owner.clone(),
            key.clone(),
            AgentExecutionRef::local("other", 8),
        ),
    ] {
        callbacks::late_preparation_shutdown(
            &mut coordinator,
            callback_owner,
            callback_key,
            callback_execution,
            WorkflowCleanupStatus::Confirmed,
        )
        .await
        .unwrap();
    }

    let (worker, _outcome) = prepared_worker(execution);
    callbacks::contain_late_preparation(&mut coordinator, owner, key.clone(), worker);
    let waiter = coordinator
        .pending
        .get_mut(&key.attempt_id)
        .unwrap()
        .late_shutdown_waiter
        .take()
        .unwrap();
    assert_eq!(waiter.await.unwrap(), WorkflowCleanupStatus::Confirmed);
}
