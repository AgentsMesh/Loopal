use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::cross_hub::RemoteSpawnOutcome;
use loopal_ipc::protocol::methods;
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

use super::{handle_spawn_agent, handle_spawn_remote_agent};
use crate::request_principal::{AgentPrincipal, TrustedMetaHubPrincipal};
use crate::types::{AgentRuntimeFacts, SpawnAuthority};
use crate::{Hub, HubUplink};

async fn managed_fixture(
    root: &std::path::Path,
) -> (
    Arc<Mutex<Hub>>,
    AgentPrincipal,
    Arc<Connection<loopal_ipc::Listening>>,
    mpsc::Receiver<Incoming>,
) {
    let (event_tx, mut event_rx) = mpsc::channel(8);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_connection, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (meta_connection, meta_rx) = Connection::new(meta_transport).into_listening();
    let (_agent_peer, agent_transport) = loopal_ipc::duplex_pair();
    let (agent_connection, _agent_rx) = Connection::new(agent_transport).into_listening();
    let mut hub = Hub::with_cwd(event_tx, root.to_path_buf());
    hub.uplink = Some(Arc::new(HubUplink::new(hub_connection, "hub-a".into())));
    let execution = hub
        .registry
        .register_connection_with_parent_execution("main", agent_connection, None, None, None)
        .unwrap();
    let facts = AgentRuntimeFacts::root(root.to_path_buf(), SpawnAuthority::default());
    assert!(hub.registry.set_runtime_facts(&execution, facts.clone()));
    hub.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    (
        Arc::new(Mutex::new(hub)),
        AgentPrincipal::new(execution, facts),
        meta_connection,
        meta_rx,
    )
}

#[tokio::test]
async fn target_encoding_and_self_target_use_local_validation() {
    let root = tempfile::tempdir().unwrap();
    let (hub, principal, _meta, _meta_rx) = managed_fixture(root.path()).await;

    let error = handle_spawn_agent(
        &hub,
        json!({"name": "child", "target_hub": "hub-b/nested"}),
        &principal,
    )
    .await
    .unwrap_err();
    assert!(error.contains("target_hub") && error.contains("cannot contain '/'"));

    let error = handle_spawn_agent(
        &hub,
        json!({"name": "child", "target_hub": "hub-a", "depth": 0}),
        &principal,
    )
    .await
    .unwrap_err();
    assert!(error.contains("depth"), "{error}");
}

#[tokio::test]
async fn non_self_target_forwards_derived_spawn_authority() {
    let root = tempfile::tempdir().unwrap();
    let (hub, principal, meta, mut meta_rx) = managed_fixture(root.path()).await;
    let responder = tokio::spawn(async move {
        let Incoming::Request { id, method, params } = meta_rx.recv().await.unwrap() else {
            panic!("expected meta spawn request");
        };
        assert_eq!(method, methods::META_SPAWN.name);
        assert_eq!(params["target_hub"], "hub-b");
        assert_eq!(params["parent"], "hub-a/main");
        assert_eq!(params["depth"], 1);
        for field in ["cwd", "resume", "fork_context"] {
            assert!(params.get(field).is_none());
        }
        meta.respond(
            id,
            RemoteSpawnOutcome::Spawned {
                response: json!({"agent_id": "remote-id"}),
            }
            .into_value(),
        )
        .await
        .unwrap();
    });

    let response = handle_spawn_agent(
        &hub,
        json!({"name": "remote-child", "target_hub": "hub-b", "prompt": "work"}),
        &principal,
    )
    .await
    .unwrap();
    responder.await.unwrap();
    assert_eq!(response["agent_id"], "remote-id");
}

#[tokio::test]
async fn trusted_remote_spawn_reaches_destination_validation() {
    let root = tempfile::tempdir().unwrap();
    let (hub, _principal, _meta, _meta_rx) = managed_fixture(root.path()).await;
    let connection = hub
        .lock()
        .await
        .uplink
        .as_ref()
        .unwrap()
        .connection()
        .clone();
    let trusted = TrustedMetaHubPrincipal::new(connection);

    let error = handle_spawn_remote_agent(&hub, json!({"name": "child"}), &trusted)
        .await
        .unwrap_err();

    assert!(error.contains("cross-hub spawn missing"), "{error}");
}
