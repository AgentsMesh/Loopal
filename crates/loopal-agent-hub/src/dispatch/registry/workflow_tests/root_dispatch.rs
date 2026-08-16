async fn root_dispatch_fixture() -> (
    Arc<Mutex<Hub>>,
    Arc<HubRequestPrincipal>,
    WorkflowCoordinatorHandle,
    tokio::task::JoinHandle<()>,
) {
    let (events, _rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let execution = hub
        .registry
        .register_connection_with_parent_execution(ROOT_AGENT_NAME, connection, None, None, None)
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = Some("session-dispatch".into());
    assert!(hub.registry.set_runtime_facts(&execution, facts.clone()));
    let (coordinator, actor) = crate::workflow::WorkflowCoordinator::spawn_disabled();
    hub.install_workflow_coordinator(coordinator.clone());
    (
        Arc::new(Mutex::new(hub)),
        Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
            execution, facts,
        ))),
        coordinator,
        actor,
    )
}
#[tokio::test]
async fn every_root_workflow_handler_reaches_the_bound_backend() {
    let (hub, principal, coordinator, actor) = root_dispatch_fixture().await;
    let dispatcher = crate::dispatch::build_hub_dispatcher(hub.clone());
    let run_id = WorkflowRunId::new("wrun_dispatch");
    let requests = [
        (
            methods::HUB_WORKFLOW_START.name,
            serde_json::to_value(start_request("wreq_dispatch_start")).unwrap(),
        ),
        (
            methods::HUB_WORKFLOW_LOOKUP_START.name,
            serde_json::to_value(WorkflowStartLookupRequest {
                request_id: WorkflowRequestId::new("wreq_dispatch_lookup"),
            })
            .unwrap(),
        ),
        (
            methods::HUB_WORKFLOW_GET.name,
            serde_json::to_value(WorkflowGetRequest {
                request_id: WorkflowRequestId::new("wreq_dispatch_get"),
                run_id: run_id.clone(),
            })
            .unwrap(),
        ),
        (
            methods::HUB_WORKFLOW_WAIT.name,
            serde_json::to_value(WorkflowWaitRequest {
                request_id: WorkflowRequestId::new("wreq_dispatch_wait"),
                run_id: run_id.clone(),
                after_revision: 0,
                timeout_ms: 1,
            })
            .unwrap(),
        ),
        (
            methods::HUB_WORKFLOW_CANCEL.name,
            serde_json::to_value(WorkflowCancelRequest {
                request_id: WorkflowRequestId::new("wreq_dispatch_cancel"),
                run_id,
                reason: Some("coverage cancellation".into()),
            })
            .unwrap(),
        ),
    ];

    for (method, params) in requests {
        let error = crate::dispatch::dispatch_hub_request_with_principal(
            &hub,
            &dispatcher,
            method,
            params,
            principal.clone(),
        )
        .await
        .unwrap_err();
        assert!(error.contains("disabled"), "{method}: {error}");
    }

    hub.lock().await.clear_workflow_coordinator();
    coordinator.shutdown().await.unwrap();
    actor.await.unwrap();
}

fn start_request(request_id: &str) -> WorkflowStartRequest {
    WorkflowStartRequest {
        request_id: WorkflowRequestId::new(request_id),
        spec: WorkflowSpec {
            version: WORKFLOW_SPEC_V1,
            run_goal: "exercise workflow dispatch".into(),
            nodes: vec![WorkflowAgentNode {
                id: WorkflowNodeId::new("wnode_dispatch"),
                dependencies: Vec::new(),
                task: "dispatch the workflow".into(),
                worker_profile: WorkflowWorkerProfileRef::new("default"),
            }],
            limits: WorkflowLimits {
                max_nodes: 4,
                max_parallel: 1,
                max_attempts: 2,
                run_deadline_ms: 30_000,
                attempt_timeout_ms: 10_000,
                max_output_bytes: 1_024,
            },
            output_node: WorkflowNodeId::new("wnode_dispatch"),
            output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
        },
    }
}
