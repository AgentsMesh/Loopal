use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::{
    SecretGetRequest, WorkflowAttemptCapability, WorkflowAttemptId, WorkflowNodeId,
    WorkflowPermissionCausation, WorkflowProviderSecretGetRequest, WorkflowRunId,
};
use secrecy::ExposeSecret;

use super::*;
use crate::HUB_RPC_BUDGET;

#[path = "tests_coverage.rs"]
mod coverage;

#[tokio::test]
async fn agent_client_uses_general_method_and_caller_identity() {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client_connection, _client_rx) = Connection::new(client_transport).into_listening();
    let (server_connection, mut server_rx) = Connection::new(server_transport).into_listening();
    let responder = tokio::spawn(async move {
        let Some(Incoming::Request { id, method, params }) = server_rx.recv().await else {
            panic!("expected agent secret request")
        };
        assert_eq!(method, methods::HUB_SECRET_GET.name);
        let request: SecretGetRequest = serde_json::from_value(params).unwrap();
        assert_eq!(request.cwd, "/workspace");
        assert_eq!(request.name, "agent_key");
        assert_eq!(request.caller.agent_name, "worker");
        assert_eq!(request.caller.depth, 2);
        assert_eq!(request.caller.tool_name, None);
        server_connection
            .respond(id, serde_json::json!({"plaintext": "agent-secret"}))
            .await
            .unwrap();
    });
    let client = HubSecretClient::new(
        client_connection,
        std::path::PathBuf::from("/workspace"),
        "worker".into(),
        2,
    );

    let secret = client.get("agent_key", HUB_RPC_BUDGET).await.unwrap();
    assert_eq!(secret.expose_secret(), "agent-secret");
    responder.await.unwrap();
}

#[tokio::test]
async fn workflow_provider_client_uses_capability_bound_method_and_shared_redaction() {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client_connection, _client_rx) = Connection::new(client_transport).into_listening();
    let (server_connection, mut server_rx) = Connection::new(server_transport).into_listening();
    let causation = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_provider_client"),
        node_id: WorkflowNodeId::new("wnode_provider_client"),
        attempt_id: WorkflowAttemptId::new("watt_provider_client"),
    };
    let capability = WorkflowAttemptCapability::parse("31".repeat(32)).unwrap();
    let expected_causation = causation.clone();
    let expected_capability = capability.clone();
    let responder = tokio::spawn(async move {
        let Some(Incoming::Request { id, method, params }) = server_rx.recv().await else {
            panic!("expected provider secret request")
        };
        assert_eq!(method, methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name);
        let request: WorkflowProviderSecretGetRequest = serde_json::from_value(params).unwrap();
        assert_eq!(request.name, "provider_key");
        assert_eq!(request.causation, expected_causation);
        assert_eq!(request.capability, expected_capability);
        server_connection
            .respond(id, serde_json::json!({"plaintext": "provider-secret"}))
            .await
            .unwrap();
    });
    let seed = FinalSinkRedactionSeed::new();
    let client = HubSecretClient::new_workflow_provider(
        client_connection,
        std::path::PathBuf::from("/workspace"),
        causation,
        capability,
    )
    .with_final_sink_redaction_seed(seed.clone());

    let secret = client.get("provider_key", HUB_RPC_BUDGET).await.unwrap();
    assert_eq!(secret.expose_secret(), "provider-secret");
    let guarded = seed.guard_completion(loopal_protocol::AgentCompletion::goal(Some(
        "provider-secret".into(),
    )));
    assert_eq!(guarded.output(), "<secret_ref:provider_key>");
    responder.await.unwrap();
}

struct DefaultMetadataClient;

#[async_trait]
impl SecretClient for DefaultMetadataClient {
    async fn get(&self, _name: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }

    async fn list_names(&self, _budget: IpcBudget) -> SecretResult<Vec<String>> {
        unreachable!()
    }

    async fn expand_author(
        &self,
        _template: &str,
        _budget: IpcBudget,
    ) -> SecretResult<SecretString> {
        unreachable!()
    }

    async fn expand_wire(&self, _template: &str, _budget: IpcBudget) -> SecretResult<SecretString> {
        unreachable!()
    }
}

#[test]
fn trait_metadata_defaults_are_absent() {
    let client = DefaultMetadataClient;
    assert!(client.health().is_none());
    assert!(client.final_sink_redaction_seed().is_none());
}
