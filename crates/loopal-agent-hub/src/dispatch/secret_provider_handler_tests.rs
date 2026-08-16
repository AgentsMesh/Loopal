use loopal_protocol::{
    SecretGetResponse, WorkflowAttemptCapability, WorkflowAttemptId, WorkflowNodeId,
    WorkflowPermissionCausation, WorkflowProviderSecretGetRequest, WorkflowRunId,
};

use super::handle_workflow_provider_secret_get;

#[tokio::test]
async fn provider_secret_is_bound_to_exact_workflow_capability_and_redacted() {
    let (temp, vault) =
        crate::mcp_service::test_vault::service(&[("provider_key", "provider-secret-value")]).await;
    let (events, _rx) = tokio::sync::mpsc::channel(8);
    let hub = std::sync::Arc::new(tokio::sync::Mutex::new(crate::Hub::with_cwd(
        events,
        temp.path().into(),
    )));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = loopal_ipc::Connection::new(transport).into_listening().0;
    let causation = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_provider_secret"),
        node_id: WorkflowNodeId::new("wnode_provider_secret"),
        attempt_id: WorkflowAttemptId::new("watt_provider_secret"),
    };
    let capability = WorkflowAttemptCapability::parse("42".repeat(32)).unwrap();
    let (execution, facts) = {
        let mut locked = hub.lock().await;
        let execution = locked
            .registry
            .register_connection_with_parent_execution(
                "workflow-worker",
                connection,
                None,
                None,
                None,
            )
            .unwrap();
        let root_execution = crate::types::AgentExecutionRef::local("root", 1);
        assert!(locked.spawn_registry.register_exact(
            root_execution.clone(),
            temp.path().into(),
            None,
        ));
        let mut facts = crate::types::AgentRuntimeFacts::root(
            temp.path().into(),
            crate::types::SpawnAuthority::default(),
        );
        facts.origin = crate::types::AgentOrigin::ManagedChild;
        facts.depth = 1;
        facts.parent = Some(root_execution.clone());
        facts.workflow_permission_causation = Some(causation.clone());
        facts.workflow_attempt_capability_digest = Some(capability.digest());
        assert!(locked.registry.set_runtime_facts(&execution, facts.clone()));
        assert!(locked.spawn_registry.register_exact(
            execution.clone(),
            temp.path().into(),
            Some(root_execution),
        ));
        locked.set_vault_service(vault);
        (execution, facts)
    };
    let principal = crate::request_principal::AgentPrincipal::new(execution, facts);
    let request = |capability| WorkflowProviderSecretGetRequest {
        cwd: temp.path().display().to_string(),
        name: "provider_key".into(),
        causation: causation.clone(),
        capability,
    };

    let response = handle_workflow_provider_secret_get(
        &hub,
        serde_json::to_value(request(capability)).unwrap(),
        &principal,
    )
    .await
    .unwrap();
    let response: SecretGetResponse = serde_json::from_value(response).unwrap();
    assert_eq!(response.plaintext, "provider-secret-value");
    let guarded = hub
        .lock()
        .await
        .final_sink_redaction_seed()
        .guard_completion(loopal_protocol::AgentCompletion::goal(Some(
            "provider-secret-value".into(),
        )));
    assert_eq!(guarded.output(), "<secret_ref:provider_key>");

    let forged = WorkflowAttemptCapability::parse("24".repeat(32)).unwrap();
    let error = handle_workflow_provider_secret_get(
        &hub,
        serde_json::to_value(request(forged)).unwrap(),
        &principal,
    )
    .await
    .unwrap_err();
    assert!(error.contains("permission_denied"));
}
