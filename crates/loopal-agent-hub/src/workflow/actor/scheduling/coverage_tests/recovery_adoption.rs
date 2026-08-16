use super::*;

#[tokio::test]
async fn recovery_adoption_covers_dispatch_success_and_rejection_matrix() {
    let owner = owner();
    let journal = Arc::new(MemoryJournal::default());
    let (run, key) = dispatching_run("wrun_adopt_dispatch", "watt_adopt_dispatch");
    let request = reconnect_request(&run, &key);
    let spawner = TestSpawner::confirmed();
    spawner.adopt_worker(request.execution.clone());
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        journal.clone(),
        spawner,
        20,
    );
    coordinator
        .recovery_deadlines
        .insert(key.attempt_id.clone(), 100);
    let response = recovery::adopt(&mut coordinator, owner.clone(), request.clone())
        .await
        .unwrap();
    assert_eq!(
        response.attempt_state,
        loopal_protocol::WorkflowAttemptState::Running
    );
    assert!(coordinator.recovered_adoptions.contains(&key.attempt_id));
    assert_eq!(journal.payloads().len(), 2);

    let (run, key) = dispatching_run("wrun_adopt_missing_deadline", "watt_adopt_missing_deadline");
    let request = reconnect_request(&run, &key);
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        20,
    );
    assert_eq!(
        recovery::adopt(&mut coordinator, owner.clone(), request).await,
        Err(WorkflowCoordinatorError::StaleExecutionLease)
    );

    for already_adopted in [false, true] {
        let (run, key) = dispatching_run(
            if already_adopted {
                "wrun_adopt_duplicate"
            } else {
                "wrun_adopt_expired"
            },
            if already_adopted {
                "watt_adopt_duplicate"
            } else {
                "watt_adopt_expired"
            },
        );
        let request = reconnect_request(&run, &key);
        let (mut coordinator, _commands) = self::coordinator(
            WorkflowCoordinatorMode::ExecutionHarness,
            true,
            vec![run],
            Arc::new(MemoryJournal::default()),
            TestSpawner::confirmed(),
            if already_adopted { 20 } else { 100 },
        );
        coordinator
            .recovery_deadlines
            .insert(key.attempt_id.clone(), 100);
        if already_adopted {
            coordinator
                .recovered_adoptions
                .insert(key.attempt_id.clone());
        }
        assert_eq!(
            recovery::adopt(&mut coordinator, owner.clone(), request).await,
            Err(WorkflowCoordinatorError::StaleExecutionLease)
        );
    }

    for (error, expected) in [
        (
            WorkflowRecoveryAdoptionError::ConflictingOwner,
            WorkflowCoordinatorError::InvalidExecutionLease,
        ),
        (
            WorkflowRecoveryAdoptionError::StaleExecution,
            WorkflowCoordinatorError::StaleExecutionLease,
        ),
        (
            WorkflowRecoveryAdoptionError::InvalidPhase,
            WorkflowCoordinatorError::StaleExecutionLease,
        ),
    ] {
        let suffix = match error {
            WorkflowRecoveryAdoptionError::ConflictingOwner => "conflict",
            WorkflowRecoveryAdoptionError::StaleExecution => "stale",
            WorkflowRecoveryAdoptionError::InvalidPhase => "phase",
            WorkflowRecoveryAdoptionError::MissingCustody => unreachable!(),
        };
        let (run, key) = dispatching_run(
            &format!("wrun_adopt_{suffix}"),
            &format!("watt_adopt_{suffix}"),
        );
        let request = reconnect_request(&run, &key);
        let (mut coordinator, _commands) = self::coordinator(
            WorkflowCoordinatorMode::ExecutionHarness,
            true,
            vec![run],
            Arc::new(MemoryJournal::default()),
            TestSpawner::adopt_error(error),
            20,
        );
        coordinator
            .recovery_deadlines
            .insert(key.attempt_id.clone(), 100);
        assert_eq!(
            recovery::adopt(&mut coordinator, owner.clone(), request).await,
            Err(expected)
        );
    }

    let (run, key) = dispatching_run("wrun_adopt_mismatch", "watt_adopt_mismatch");
    let request = reconnect_request(&run, &key);
    let spawner = TestSpawner::confirmed();
    spawner.adopt_worker(AgentExecutionRef::local("different-worker", 99));
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![run],
        Arc::new(MemoryJournal::default()),
        spawner,
        20,
    );
    coordinator
        .recovery_deadlines
        .insert(key.attempt_id.clone(), 100);
    assert_eq!(
        recovery::adopt(&mut coordinator, owner.clone(), request).await,
        Err(WorkflowCoordinatorError::InvalidExecutionLease)
    );
    assert!(coordinator.state.is_poisoned(&owner));

    for (mode, recovered, poison, invalid_owner, expected) in [
        (
            WorkflowCoordinatorMode::Preview,
            true,
            false,
            false,
            WorkflowCoordinatorError::InvalidOwner,
        ),
        (
            WorkflowCoordinatorMode::ExecutionHarness,
            true,
            false,
            true,
            WorkflowCoordinatorError::InvalidOwner,
        ),
        (
            WorkflowCoordinatorMode::ExecutionHarness,
            true,
            true,
            false,
            WorkflowCoordinatorError::OwnerPoisoned,
        ),
        (
            WorkflowCoordinatorMode::ExecutionHarness,
            false,
            false,
            false,
            WorkflowCoordinatorError::RecoveryRequired,
        ),
    ] {
        let (run, key) = dispatching_run("wrun_adopt_owner", "watt_adopt_owner");
        let request = reconnect_request(&run, &key);
        let (mut coordinator, _commands) = self::coordinator(
            mode,
            recovered,
            if recovered { vec![run] } else { Vec::new() },
            Arc::new(MemoryJournal::default()),
            TestSpawner::confirmed(),
            20,
        );
        if poison {
            coordinator.state.poison(owner.clone());
        }
        let request_owner = if invalid_owner {
            WorkflowOwner::new("", QualifiedAddress::local("root"))
        } else {
            owner.clone()
        };
        assert_eq!(
            recovery::adopt(&mut coordinator, request_owner, request).await,
            Err(expected)
        );
    }
}
