use loopal_config::{ConfigResolver, ProviderConfig};
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;

use super::build;
use crate::params::StartParams;
use crate::session_hub::SessionHub;

fn config() -> loopal_config::ResolvedConfig {
    ConfigResolver::new().resolve().unwrap()
}

#[tokio::test]
async fn production_sub_kernel_uses_hub_secret_with_sub_identity() {
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_incoming) = Connection::new(agent_transport).into_listening();
    let (hub, mut hub_incoming) = Connection::new(hub_transport).into_listening();
    let responder = tokio::spawn(async move {
        loop {
            let Some(Incoming::Request { id, method, params }) = hub_incoming.recv().await else {
                panic!("expected secret request")
            };
            if method == methods::HUB_SECRET_GET.name {
                hub.respond(id, serde_json::json!({"plaintext": "resolved-key"}))
                    .await
                    .unwrap();
                break params;
            }
            assert_eq!(method, methods::HUB_MCP_LIST_TOOLS.name);
            hub.respond(id, serde_json::json!({"tools": []}))
                .await
                .unwrap();
        }
    });
    let mut config = config();
    config.settings.providers.anthropic = Some(ProviderConfig {
        api_key: Some("{{secret:token}}".into()),
        api_key_env: None,
        base_url: None,
    });
    let start = StartParams {
        depth: Some(1),
        ..Default::default()
    };
    let cwd = tempfile::tempdir().unwrap();

    let kernel = build(
        &agent,
        &config,
        &start,
        cwd.path(),
        "session",
        &SessionHub::new(),
        true,
    )
    .await
    .unwrap();

    assert!(kernel.secret_client().is_some());
    assert!(kernel.mcp_manager().is_none());
    let request = responder.await.unwrap();
    assert_eq!(request["name"], "token");
    assert_eq!(request["caller"]["agent_name"], "sub");
    assert_eq!(request["caller"]["depth"], 1);
}

#[tokio::test]
async fn workflow_kernel_uses_capability_bound_provider_secret_method() {
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_incoming) = Connection::new(agent_transport).into_listening();
    let (hub, mut hub_incoming) = Connection::new(hub_transport).into_listening();
    let causation = loopal_protocol::WorkflowPermissionCausation {
        run_id: "wrun_kernel_provider".into(),
        node_id: "wnode_kernel_provider".into(),
        attempt_id: "watt_kernel_provider".into(),
    };
    let capability = loopal_protocol::WorkflowAttemptCapability::parse("51".repeat(32)).unwrap();
    let expected_causation = causation.clone();
    let expected_capability = capability.clone();
    let responder = tokio::spawn(async move {
        loop {
            let Some(Incoming::Request { id, method, params }) = hub_incoming.recv().await else {
                panic!("expected workflow provider secret request")
            };
            match method.as_str() {
                method if method == methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name => {
                    hub.respond(id, serde_json::json!({"plaintext": "workflow-key"}))
                        .await
                        .unwrap();
                    break params;
                }
                method if method == methods::HUB_MCP_LIST_TOOLS.name => {
                    hub.respond(id, serde_json::json!({"tools": []}))
                        .await
                        .unwrap();
                }
                other => panic!("unexpected startup method {other}"),
            }
        }
    });
    let mut config = config();
    config.settings.providers.anthropic = Some(ProviderConfig {
        api_key: Some("{{secret:provider_key}}".into()),
        api_key_env: None,
        base_url: None,
    });
    let start = StartParams {
        depth: Some(1),
        workflow_permission_causation: Some(causation),
        workflow_attempt_capability: Some(capability),
        ..Default::default()
    };
    let cwd = tempfile::tempdir().unwrap();

    let kernel = build(
        &agent,
        &config,
        &start,
        cwd.path(),
        "session",
        &SessionHub::new(),
        true,
    )
    .await
    .unwrap();
    let request: loopal_protocol::WorkflowProviderSecretGetRequest =
        serde_json::from_value(responder.await.unwrap()).unwrap();
    assert_eq!(request.name, "provider_key");
    assert_eq!(request.causation, expected_causation);
    assert_eq!(request.capability, expected_capability);
    let seed = kernel
        .secret_client()
        .unwrap()
        .final_sink_redaction_seed()
        .unwrap();
    let guarded = seed.guard_completion(loopal_protocol::AgentCompletion::goal(Some(
        "workflow-key".into(),
    )));
    assert_eq!(guarded.output(), "<secret_ref:provider_key>");
}

#[tokio::test]
async fn nonproduction_without_injected_provider_builds_fallback_kernel() {
    let (agent_transport, _hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _incoming) = Connection::new(agent_transport).into_listening();
    let cwd = tempfile::tempdir().unwrap();

    let kernel = build(
        &agent,
        &config(),
        &StartParams::default(),
        cwd.path(),
        "session",
        &SessionHub::new(),
        false,
    )
    .await
    .unwrap();

    assert!(!kernel.tool_definitions().is_empty());
    assert!(kernel.mcp_manager().is_some());
}
