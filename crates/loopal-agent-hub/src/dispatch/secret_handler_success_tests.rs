use loopal_protocol::{
    SecretCaller, SecretGetRequest, SecretGetResponse, SecretHealthRequest, SecretHealthResponse,
    SecretListNamesRequest, SecretListNamesResponse,
};

use super::{handle_secret_get, handle_secret_health, handle_secret_list_names};

#[tokio::test]
async fn get_list_and_health_succeed_for_current_exact_agent() {
    let (temp, vault) = crate::mcp_service::test_vault::service(&[
        ("api_key", "secret-value"),
        ("token", "wire-value"),
    ])
    .await;
    let (events, _rx) = tokio::sync::mpsc::channel(8);
    let hub = std::sync::Arc::new(tokio::sync::Mutex::new(crate::Hub::with_cwd(
        events,
        temp.path().into(),
    )));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = loopal_ipc::Connection::new(transport).into_listening().0;
    let mut locked = hub.lock().await;
    let execution = locked
        .registry
        .register_connection_with_parent_execution("agent", connection, None, None, None)
        .unwrap();
    let facts = crate::types::AgentRuntimeFacts::root(
        temp.path().into(),
        crate::types::SpawnAuthority::default(),
    );
    assert!(locked.registry.set_runtime_facts(&execution, facts.clone()));
    assert!(
        locked
            .spawn_registry
            .register_exact(execution.clone(), temp.path().into(), None)
    );
    locked.set_vault_service(vault);
    drop(locked);
    let principal = crate::request_principal::AgentPrincipal::new(execution, facts);
    assert_eq!(principal.depth, 0);

    let get = handle_secret_get(
        &hub,
        serde_json::to_value(SecretGetRequest {
            cwd: temp.path().display().to_string(),
            name: "api_key".into(),
            caller: SecretCaller {
                agent_name: principal.execution.address.agent.clone(),
                depth: principal.depth,
                tool_name: Some("Bash".into()),
            },
        })
        .unwrap(),
        &principal,
    )
    .await
    .unwrap();
    let get: SecretGetResponse = serde_json::from_value(get).unwrap();
    assert_eq!(get.plaintext, "secret-value");
    let guarded = hub
        .lock()
        .await
        .final_sink_redaction_seed()
        .guard_completion(loopal_protocol::AgentCompletion::goal(Some(
            "returned secret-value".into(),
        )));
    assert_eq!(guarded.output(), "returned <secret_ref:api_key>");

    let list = handle_secret_list_names(
        &hub,
        serde_json::to_value(SecretListNamesRequest {
            cwd: temp.path().display().to_string(),
        })
        .unwrap(),
        &principal,
    )
    .await
    .unwrap();
    let mut list: SecretListNamesResponse = serde_json::from_value(list).unwrap();
    list.names.sort();
    assert_eq!(list.names, ["api_key", "token"]);

    let health = handle_secret_health(
        &hub,
        serde_json::to_value(SecretHealthRequest {
            cwd: temp.path().display().to_string(),
        })
        .unwrap(),
        &principal,
    )
    .await
    .unwrap();
    let health: SecretHealthResponse = serde_json::from_value(health).unwrap();
    assert_eq!(health.vault_count, 1);
    assert_eq!(health.default_vault, "default");
    assert!(health.last_op_ts > 0);
}
