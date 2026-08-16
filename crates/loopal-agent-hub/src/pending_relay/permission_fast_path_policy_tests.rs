use super::*;
use crate::types::{AgentExecutionRef, AgentOrigin, AgentRuntimeFacts};

fn assert_policy_denied(
    hub: &mut Hub,
    execution: &AgentExecutionRef,
    seed: &loopal_protocol::PermissionIntentSeed,
    facts: AgentRuntimeFacts,
) {
    assert!(hub.registry.set_runtime_facts(execution, facts));
    assert!(!super::super::policy_workflow_authorized(
        hub, seed, execution
    ));
}

#[tokio::test]
async fn non_bypass_workflow_without_ui_gets_no_policy_receipt() {
    let (events, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    hub.lock()
        .await
        .set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    let (_root, _root_rx) = hub_server::connect_local(hub.clone(), "main");
    let workflow = workflow();
    let (agent, _) = connect_worker(&hub, workflow.clone(), PermissionMode::AskDangerous).await;
    let response = agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            serde_json::to_value(request(workflow)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response, json!({"allow": false}));
    assert_eq!(hub.lock().await.permission_receipts.len(), 0);
}

#[tokio::test]
async fn workflow_policy_rejects_each_stale_or_malformed_runtime_fact() {
    let (events, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (_root, _root_rx) = hub_server::connect_local(hub.clone(), "main");
    let causation = workflow();
    let (_agent, execution) = connect_worker(&hub, causation.clone(), PermissionMode::Bypass).await;
    let seed = request(causation).intent_seed;
    let mut hub = hub.lock().await;
    let baseline = hub.registry.runtime_facts(&execution).unwrap().clone();
    let parent = baseline.parent.clone().unwrap();
    let parent_facts = hub.registry.runtime_facts(&parent).unwrap().clone();
    assert!(super::super::policy_workflow_authorized(
        &hub, &seed, &execution
    ));

    let mut facts = baseline.clone();
    facts.origin = AgentOrigin::ExternalTcp;
    assert_policy_denied(&mut hub, &execution, &seed, facts);
    let mut facts = baseline.clone();
    facts.depth = 0;
    assert_policy_denied(&mut hub, &execution, &seed, facts);
    let mut facts = baseline.clone();
    facts.spawn.permission_mode = PermissionMode::AskDangerous;
    assert_policy_denied(&mut hub, &execution, &seed, facts);
    let mut facts = baseline.clone();
    facts.workflow_permission_causation = Some(WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_other"),
        node_id: WorkflowNodeId::new("wnode_other"),
        attempt_id: WorkflowAttemptId::new("watt_other"),
    });
    assert_policy_denied(&mut hub, &execution, &seed, facts);
    let mut facts = baseline.clone();
    facts.workflow_attempt_capability_digest = None;
    assert_policy_denied(&mut hub, &execution, &seed, facts);
    let mut facts = baseline.clone();
    facts.parent = Some(AgentExecutionRef::local(
        "main",
        parent.connection_generation + 1,
    ));
    assert_policy_denied(&mut hub, &execution, &seed, facts);
    let mut facts = baseline.clone();
    facts.root = "other".into();
    assert_policy_denied(&mut hub, &execution, &seed, facts);

    assert!(hub.registry.set_runtime_facts(&execution, baseline.clone()));
    hub.registry.agents.get_mut("main").unwrap().runtime = None;
    assert!(!super::super::policy_workflow_authorized(
        &hub, &seed, &execution
    ));
    assert!(
        hub.registry
            .set_runtime_facts(&parent, parent_facts.clone())
    );
    let mut invalid_parent = parent_facts.clone();
    invalid_parent.session_id = None;
    assert!(hub.registry.set_runtime_facts(&parent, invalid_parent));
    assert!(!super::super::policy_workflow_authorized(
        &hub, &seed, &execution
    ));

    assert!(hub.registry.set_runtime_facts(&parent, parent_facts));
    hub.registry.agents.get_mut("worker").unwrap().runtime = None;
    assert!(!super::super::policy_workflow_authorized(
        &hub, &seed, &execution
    ));
    assert!(!super::super::policy_workflow_authorized(
        &hub,
        &seed,
        &AgentExecutionRef::local("worker", execution.connection_generation + 1,),
    ));
}
