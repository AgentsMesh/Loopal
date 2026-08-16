fn causation(suffix: &str) -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new(format!("wrun_{suffix}")),
        node_id: WorkflowNodeId::new(format!("wnode_{suffix}")),
        attempt_id: WorkflowAttemptId::new(format!("watt_{suffix}")),
    }
}
struct WorkerFixture {
    hub: Arc<Mutex<Hub>>,
    principal: AgentPrincipal,
    request: WorkflowWorkerHandshakeRequest,
    root: AgentExecutionRef,
    worker: AgentExecutionRef,
    facts: AgentRuntimeFacts,
    coordinator: Option<WorkflowCoordinatorHandle>,
    actor: Option<tokio::task::JoinHandle<()>>,
}

async fn worker_fixture(backend: bool, install_facts: bool) -> WorkerFixture {
    let (events, _rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    let connection = || {
        let (_peer, transport) = loopal_ipc::duplex_pair();
        Connection::new(transport).into_listening().0
    };
    let root = hub
        .registry
        .register_connection_with_parent_execution(ROOT_AGENT_NAME, connection(), None, None, None)
        .unwrap();
    let mut root_facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    root_facts.session_id = Some("session-worker".into());
    assert!(hub.registry.set_runtime_facts(&root, root_facts));
    let worker = hub
        .registry
        .register_connection_with_exact_parent_execution(
            "workflow-worker",
            connection(),
            Some(QualifiedAddress::local(ROOT_AGENT_NAME)),
            Some(&root),
            None,
            None,
            false,
        )
        .unwrap();
    let workflow = causation("handshake");
    let capability = WorkflowAttemptCapability::parse("33".repeat(32)).unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.origin = AgentOrigin::ManagedChild;
    facts.parent = Some(root.clone());
    facts.depth = 1;
    facts.workflow_permission_causation = Some(workflow.clone());
    facts.workflow_attempt_capability_digest = Some(capability.digest());
    if install_facts {
        assert!(hub.registry.set_runtime_facts(&worker, facts.clone()));
    }
    let (coordinator, actor) = if backend {
        let (coordinator, actor) = crate::workflow::WorkflowCoordinator::spawn_disabled();
        hub.install_workflow_coordinator(coordinator.clone());
        (Some(coordinator), Some(actor))
    } else {
        (None, None)
    };
    WorkerFixture {
        hub: Arc::new(Mutex::new(hub)),
        principal: AgentPrincipal::new(worker.clone(), facts.clone()),
        request: WorkflowWorkerHandshakeRequest {
            causation: workflow,
            capability,
        },
        root,
        worker,
        facts,
        coordinator,
        actor,
    }
}

async fn stop_worker_fixture(fixture: &mut WorkerFixture) {
    if let Some(coordinator) = fixture.coordinator.take() {
        fixture.hub.lock().await.clear_workflow_coordinator();
        coordinator.shutdown().await.unwrap();
    }
    if let Some(actor) = fixture.actor.take() {
        actor.await.unwrap();
    }
}
