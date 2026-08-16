use std::sync::Arc;

use loopal_config::SandboxPolicy;
use loopal_decision_api::DecisionMode;
use loopal_ipc::Connection;
use loopal_tool_api::PermissionMode;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::spawn_prepare::prepare_remote_spawn;
use crate::{Hub, HubUplink};

fn payload() -> Value {
    json!({
        "name": "child",
        "model": "worker-model",
        "parent": "origin/parent",
        "depth": 2,
        "permission_mode": "bypass",
        "decision_mode": "classifier",
        "sandbox_policy": "disabled",
        "no_sandbox": true,
    })
}

fn hub_and_lease() -> (Hub, Arc<Connection<loopal_ipc::connection::Listening>>) {
    let root = tempfile::tempdir().unwrap().keep();
    let (events, _receiver) = mpsc::channel(8);
    let mut hub = Hub::with_cwd(events, root);
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_connection, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (_meta_connection, _meta_rx) = Connection::new(meta_transport).into_listening();
    hub.uplink = Some(Arc::new(HubUplink::new(
        hub_connection.clone(),
        "destination".into(),
    )));
    (hub, hub_connection)
}

#[tokio::test]
async fn destination_ceiling_cannot_be_relaxed_by_origin() {
    let (mut hub, connection) = hub_and_lease();
    let settings = loopal_config::Settings {
        permission_mode: PermissionMode::AskAnyWrite,
        decision_mode: DecisionMode::Manual,
        sandbox: loopal_config::SandboxConfig {
            policy: SandboxPolicy::ReadOnly,
            ..Default::default()
        },
        ..Default::default()
    };
    hub.set_root_spawn_authority(&settings);

    let prepared = prepare_remote_spawn(&payload(), &hub, connection).unwrap();
    assert_eq!(
        prepared.authority.permission_mode,
        PermissionMode::AskAnyWrite
    );
    assert_eq!(prepared.authority.decision_mode, DecisionMode::Manual);
    assert_eq!(prepared.authority.sandbox_policy, SandboxPolicy::ReadOnly);
    assert!(prepared.request_lease.is_current(&hub));
}

#[tokio::test]
async fn stricter_origin_policy_survives_permissive_destination() {
    let (mut hub, connection) = hub_and_lease();
    let settings = loopal_config::Settings {
        permission_mode: PermissionMode::Bypass,
        decision_mode: DecisionMode::Classifier,
        sandbox: loopal_config::SandboxConfig {
            policy: SandboxPolicy::Disabled,
            ..Default::default()
        },
        ..Default::default()
    };
    hub.set_root_spawn_authority(&settings);
    let mut incoming = payload();
    incoming["permission_mode"] = json!("ask_dangerous");
    incoming["sandbox_policy"] = json!("read_only");
    incoming["no_sandbox"] = json!(false);

    let prepared = prepare_remote_spawn(&incoming, &hub, connection).unwrap();
    assert_eq!(
        prepared.authority.permission_mode,
        PermissionMode::AskDangerous
    );
    assert_eq!(prepared.authority.sandbox_policy, SandboxPolicy::ReadOnly);
}

#[tokio::test]
async fn destination_rejects_depth_over_local_ceiling() {
    let (mut hub, connection) = hub_and_lease();
    hub.max_agent_depth = 1;
    let error = prepare_remote_spawn(&payload(), &hub, connection)
        .err()
        .expect("depth above destination ceiling must fail");
    assert!(error.contains("depth"));
}

#[tokio::test]
async fn prepared_remote_spawn_is_bound_to_active_uplink() {
    let (mut hub, connection) = hub_and_lease();
    let prepared = prepare_remote_spawn(&payload(), &hub, connection).unwrap();
    hub.uplink = None;
    assert!(!prepared.request_lease.is_current(&hub));
}
