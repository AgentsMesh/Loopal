use super::*;

#[tokio::test]
async fn resume_covers_guards_skip_reasons_and_poisoning_failure() {
    let owner = owner();
    for (mode, recovered, poison, invalid_owner, expected) in [
        (
            WorkflowCoordinatorMode::Preview,
            true,
            false,
            false,
            WorkflowCoordinatorError::Disabled,
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
        let (mut coordinator, _commands) = self::coordinator(
            mode,
            recovered,
            Vec::new(),
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
        assert_eq!(coordinator.resume_owner(request_owner).await, Err(expected));
    }

    let mut no_ready = running_ready_run("wrun_resume_no_ready");
    no_ready.nodes[0].state = WorkflowNodeState::Pending;
    let (dispatching, _) = dispatching_run("wrun_resume_busy", "watt_resume_busy");
    let planned = planned_run("wrun_resume_planned");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![no_ready, dispatching, planned],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        30,
    );
    coordinator.resume_owner(owner.clone()).await.unwrap();
    assert!(coordinator.resumed_owners.contains(&owner));

    let validated = validated_run("wrun_resume_pending_guard");
    let guard_key = AttemptKey {
        run_id: validated.id.clone(),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new("watt_resume_pending_guard"),
    };
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![validated],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        40,
    );
    coordinator
        .pending
        .insert(guard_key.attempt_id.clone(), pending(&owner, &guard_key));
    coordinator.resume_owner(owner.clone()).await.unwrap();

    let validated = validated_run("wrun_resume_active_guard");
    let guard_key = AttemptKey {
        run_id: validated.id.clone(),
        node_id: WorkflowNodeId::new("node"),
        attempt_id: WorkflowAttemptId::new("watt_resume_active_guard"),
    };
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![validated],
        Arc::new(MemoryJournal::default()),
        TestSpawner::confirmed(),
        50,
    );
    coordinator.active.insert(
        guard_key.attempt_id.clone(),
        active(
            &owner,
            &guard_key,
            AgentExecutionRef::local("resume-active", 15),
            ActiveAttemptPhase::Running,
        ),
    );
    coordinator.resume_owner(owner.clone()).await.unwrap();

    let validated = validated_run("wrun_resume_failure");
    let (mut coordinator, _commands) = self::coordinator(
        WorkflowCoordinatorMode::ExecutionHarness,
        true,
        vec![validated],
        MemoryJournal::failing(),
        TestSpawner::confirmed(),
        60,
    );
    assert_eq!(
        coordinator.resume_owner(owner.clone()).await,
        Err(WorkflowCoordinatorError::JournalUnavailable)
    );
    assert!(coordinator.state.is_poisoned(&owner));
}
