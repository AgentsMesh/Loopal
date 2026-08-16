use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, SecretCaller, SecretGetRequest};
use loopal_secret_client::SecretError;
use tokio::sync::{Mutex, mpsc};

use super::{
    audit_ctx, handle_secret_get, handle_secret_health, handle_secret_list_names, map_err,
};
use crate::Hub;
use crate::request_principal::AgentPrincipal;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

async fn fixture() -> (Arc<Mutex<Hub>>, AgentPrincipal, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let (events, _rx) = mpsc::channel::<AgentEvent>(8);
    let hub = Arc::new(Mutex::new(Hub::with_cwd(events, temp.path().into())));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let mut locked = hub.lock().await;
    let execution = locked
        .registry
        .register_connection_with_parent_execution("agent", connection, None, None, None)
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(temp.path().into(), SpawnAuthority::default());
    facts.session_id = Some("authenticated-session".into());
    locked.registry.set_runtime_facts(&execution, facts.clone());
    locked
        .spawn_registry
        .register_exact(execution.clone(), temp.path().into(), None);
    let principal = AgentPrincipal::new(execution, facts);
    drop(locked);
    (hub, principal, temp)
}

#[tokio::test]
async fn secret_audit_context_uses_authenticated_session() {
    let (_hub, principal, _temp) = fixture().await;
    let caller = SecretCaller {
        agent_name: principal.execution.address.agent.clone(),
        depth: principal.depth,
        tool_name: Some("Bash".into()),
    };
    let context = audit_ctx(&principal, &caller);
    assert_eq!(context.session_id.as_deref(), Some("authenticated-session"));
    assert_eq!(context.agent_name, caller.agent_name);
}

#[tokio::test]
async fn malformed_caller_and_vault_state_fail_closed() {
    let (hub, principal, temp) = fixture().await;
    let malformed = handle_secret_get(&hub, serde_json::Value::Null, &principal)
        .await
        .unwrap_err();
    assert!(malformed.contains("invalid secret_get params"));
    let wrong_caller = serde_json::to_value(SecretGetRequest {
        cwd: temp.path().display().to_string(),
        name: "key".into(),
        caller: SecretCaller {
            agent_name: "other".into(),
            depth: principal.depth,
            tool_name: Some("Bash".into()),
        },
    })
    .unwrap();
    assert!(
        handle_secret_get(&hub, wrong_caller, &principal)
            .await
            .unwrap_err()
            .contains("permission_denied")
    );

    let cwd = serde_json::json!({"cwd": temp.path().display().to_string()});
    for error in [
        handle_secret_list_names(&hub, cwd.clone(), &principal).await,
        handle_secret_health(&hub, cwd, &principal).await,
    ] {
        assert!(error.unwrap_err().contains("vault service not initialized"));
    }
}

#[test]
fn every_secret_error_has_structured_wire_mapping() {
    let errors = [
        SecretError::SecretNotFound("name".into()),
        SecretError::VaultNotFound("/tmp".into()),
        SecretError::PermissionDenied,
        SecretError::DecryptFailed("detail".into()),
        SecretError::InvalidName("name".into()),
        SecretError::TemplateParse("detail".into()),
        SecretError::Ipc("detail".into()),
    ];
    for error in errors {
        assert!(serde_json::from_str::<loopal_protocol::SecretIpcError>(&map_err(error)).is_ok());
    }
}

#[tokio::test]
async fn malformed_list_and_health_params_are_rejected() {
    let (hub, principal, _temp) = fixture().await;
    let list = handle_secret_list_names(&hub, serde_json::Value::Null, &principal)
        .await
        .unwrap_err();
    let health = handle_secret_health(&hub, serde_json::Value::Null, &principal)
        .await
        .unwrap_err();
    assert!(list.contains("invalid secret_list_names params"));
    assert!(health.contains("invalid secret_health params"));
}

#[tokio::test]
async fn depth_cwd_and_stale_lease_mismatches_are_denied() {
    let (hub, principal, temp) = fixture().await;
    let request = |cwd: &std::path::Path, depth| {
        serde_json::to_value(SecretGetRequest {
            cwd: cwd.display().to_string(),
            name: "key".into(),
            caller: SecretCaller {
                agent_name: principal.execution.address.agent.clone(),
                depth,
                tool_name: None,
            },
        })
        .unwrap()
    };
    assert!(
        handle_secret_get(&hub, request(temp.path(), principal.depth + 1), &principal)
            .await
            .unwrap_err()
            .contains("permission_denied")
    );

    let outside = tempfile::tempdir().unwrap();
    assert!(
        handle_secret_get(&hub, request(outside.path(), principal.depth), &principal)
            .await
            .unwrap_err()
            .contains("permission_denied")
    );

    assert!(
        hub.lock()
            .await
            .registry
            .unregister_exact(&principal.execution)
    );
    let params = serde_json::json!({"cwd": temp.path().display().to_string()});
    for error in [
        handle_secret_list_names(&hub, params.clone(), &principal).await,
        handle_secret_health(&hub, params, &principal).await,
    ] {
        assert!(error.unwrap_err().contains("permission_denied"));
    }
}
