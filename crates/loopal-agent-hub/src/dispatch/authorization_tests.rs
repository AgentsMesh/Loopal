use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::UiCapabilities;
use tokio::sync::{Mutex, mpsc};

use super::authorization;
use crate::request_principal::{
    AgentPrincipal, HubRequestPrincipal, TrustedMetaHubPrincipal, UiPrincipal,
};
use crate::types::{AgentRuntimeFacts, SpawnAuthority};
use crate::{Hub, HubUplink};

fn connection() -> Arc<Connection<loopal_ipc::connection::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

fn hub() -> Arc<Mutex<Hub>> {
    let (events, _rx) = mpsc::channel(8);
    Arc::new(Mutex::new(Hub::new(events)))
}

fn ui_principal(
    connection: Arc<Connection<loopal_ipc::connection::Listening>>,
    capabilities: UiCapabilities,
) -> Arc<HubRequestPrincipal> {
    Arc::new(HubRequestPrincipal::Ui(UiPrincipal::new(
        "lease".into(),
        "desktop".into(),
        capabilities,
        connection,
    )))
}

#[tokio::test]
async fn ui_principal_requires_current_connection_and_capability_snapshot() {
    let hub = hub();
    let current = connection();
    let stale = connection();
    hub.lock().await.ui.register_client_with_lease(
        "lease",
        "desktop",
        current.clone(),
        UiCapabilities::ALL,
    );

    let context = authorization::authorize(
        &hub,
        methods::HUB_STATUS.name,
        ui_principal(current.clone(), UiCapabilities::ALL),
    )
    .await
    .unwrap();
    let extracted = authorization::ui(&context).unwrap();
    assert!(extracted.matches_connection(&current));

    for principal in [
        ui_principal(stale, UiCapabilities::ALL),
        ui_principal(current, UiCapabilities::NONE),
    ] {
        let error = match authorization::authorize(&hub, methods::HUB_STATUS.name, principal).await
        {
            Ok(_) => panic!("stale UI principal was authorized"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("not authorized"));
    }
}

#[tokio::test]
async fn revoked_ui_lease_is_rejected() {
    let hub = hub();
    let current = connection();
    hub.lock().await.ui.register_client_with_lease(
        "lease",
        "desktop",
        current.clone(),
        UiCapabilities::ALL,
    );
    let principal = ui_principal(current, UiCapabilities::ALL);
    hub.lock().await.ui.unregister_client("lease");

    let error = match authorization::authorize(&hub, methods::HUB_STATUS.name, principal).await {
        Ok(_) => panic!("revoked UI principal was authorized"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("not authorized"));
}

#[tokio::test]
async fn only_authenticated_root_may_reconnect_hub_mcp() {
    let hub = hub();
    let current = connection();
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("main", current, None, None, None)
        .unwrap();
    let facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts.clone())
    );
    let root = Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
        execution.clone(),
        facts,
    )));
    assert!(
        authorization::authorize(&hub, methods::HUB_MCP_RECONNECT.name, root)
            .await
            .is_ok()
    );

    hub.lock().await.registry.unregister_exact(&execution);
    let stale = Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
        execution,
        AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default()),
    )));
    assert!(
        authorization::authorize(&hub, methods::HUB_MCP_RECONNECT.name, stale)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn trusted_metahub_principal_requires_active_uplink_connection() {
    let hub = hub();
    let active = connection();
    let stale = connection();
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(active.clone(), "local".into())));

    let stale_principal = Arc::new(HubRequestPrincipal::TrustedMetaHub(
        TrustedMetaHubPrincipal::new(stale),
    ));
    let error =
        match authorization::authorize(&hub, methods::HUB_REMOTE_RELAY.name, stale_principal).await
        {
            Ok(_) => panic!("stale MetaHub principal was authorized"),
            Err(error) => error,
        };
    assert!(error.to_string().contains("not authorized"));

    let active_principal = Arc::new(HubRequestPrincipal::TrustedMetaHub(
        TrustedMetaHubPrincipal::new(active.clone()),
    ));
    let context = authorization::authorize(&hub, methods::HUB_REMOTE_RELAY.name, active_principal)
        .await
        .unwrap();
    assert!(
        authorization::trusted_meta(&context)
            .unwrap()
            .matches_connection(&active)
    );
}

#[test]
fn principal_extractors_fail_closed_for_missing_or_wrong_type() {
    let empty = loopal_ipc::HandlerCtx::new("none");
    assert!(authorization::principal(&empty).is_err());

    let context = loopal_ipc::HandlerCtx::new("internal")
        .with_extension(Arc::new(HubRequestPrincipal::Internal));
    assert!(authorization::ui(&context).is_err());
    assert!(authorization::agent(&context).is_err());
    assert!(authorization::trusted_meta(&context).is_err());
}
