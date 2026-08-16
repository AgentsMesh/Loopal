fn coordinator(
    mode: WorkflowCoordinatorMode,
    recovered: bool,
    runs: Vec<WorkflowRunSnapshot>,
    journal: Arc<dyn WorkflowJournalStorage>,
    spawner: Arc<dyn WorkflowSpawner>,
    now: u64,
) -> (WorkflowCoordinator, mpsc::Sender<WorkflowCommand>) {
    let owner = owner();
    let mut state = WorkflowActorState::new();
    if recovered {
        state
            .install_recovered(
                owner,
                RecoveredOwner {
                    runs,
                    requests: Default::default(),
                    delivery_intents: Vec::new(),
                    acked_deliveries: Default::default(),
                },
            )
            .unwrap();
    }
    let (commands, receiver) = mpsc::channel(32);
    let coordinator = WorkflowCoordinator {
        mode,
        clock: Arc::new(TestClock::new(now)),
        ids: Arc::new(SystemWorkflowIdSource),
        journal,
        commands: receiver,
        state,
        spawner,
        active: Default::default(),
        pending: Default::default(),
        callbacks: commands.downgrade(),
        cancel_grace_ms: WorkflowRuntimeConfig::test_default().cancel_grace_ms,
        trusted_ceilings: WorkflowTrustedCeilings::PROTOCOL_MAXIMUM,
        recovery_grace_ms: 100,
        recovery_deadlines: Default::default(),
        recovered_adoptions: Default::default(),
        resumed_owners: Default::default(),
        terminal_deliveries: Default::default(),
        terminal_delivery_payloads: Default::default(),
        terminal_delivery_owners: Default::default(),
        terminal_delivery_failure: None,
        revisions: Default::default(),
        event_sink: None,
        terminal_sink: Arc::new(UnavailableWorkflowTerminalSink),
        redaction_seed: loopal_output_guard::FinalSinkRedactionSeed::new(),
    };
    (coordinator, commands)
}
fn pending(owner: &WorkflowOwner, key: &AttemptKey) -> PendingAttempt {
    PendingAttempt {
        owner: owner.clone(),
        key: key.clone(),
        causation: WorkflowPermissionCausation {
            run_id: key.run_id.clone(),
            node_id: key.node_id.clone(),
            attempt_id: key.attempt_id.clone(),
        },
        deadline_unix_ms: 100,
        prepare_abort: None,
        abort_waiter: None,
        abort_requested: false,
        abort_status: None,
        delivery_finished: false,
        late_execution: None,
        late_shutdown_waiter: None,
        stop: None,
    }
}
