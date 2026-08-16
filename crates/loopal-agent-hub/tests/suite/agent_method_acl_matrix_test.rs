use std::collections::BTreeSet;
use std::sync::Arc;

use loopal_agent_hub::Hub;
use loopal_agent_hub::dispatch::build_hub_dispatcher;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::{RpcError, jsonrpc};
use tokio::sync::{Mutex, mpsc};

const MANAGED_AGENT_METHODS: &[&str] = &[
    "hub/agent_info",
    "hub/audit/permission_decision",
    "hub/audit/protected_effect",
    "hub/list_agents",
    "hub/mcp/call_tool",
    "hub/mcp/list_tools",
    "hub/mcp/snapshot",
    "hub/route",
    "hub/secret/get",
    "hub/secret/health",
    "hub/secret/list_names",
    "hub/spawn_agent",
    "hub/status",
    "hub/topology",
    "hub/wait_agent",
];

async fn managed_child() -> (
    Arc<Mutex<Hub>>,
    Arc<Connection<loopal_ipc::Listening>>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (event_tx, mut event_rx) = mpsc::channel(64);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client, client_rx) = Connection::new(client_transport).into_listening();
    let (server, server_rx) = Connection::new(server_transport).into_listening();
    register_agent_connection(hub.clone(), "child", server, server_rx, None, None, None)
        .await
        .unwrap();
    (hub, client, client_rx)
}

fn assert_central_denial(method: &str, error: RpcError) {
    assert!(
        error.to_string().contains("not authorized"),
        "{method} bypassed central authorization: {error}"
    );
}

#[tokio::test]
async fn every_registered_method_is_closed_to_managed_child_by_default() {
    let (hub, child, _child_rx) = managed_child().await;
    let dispatcher = build_hub_dispatcher(hub.clone());
    let registered: BTreeSet<_> = dispatcher.registered_methods().into_iter().collect();
    let allowed: BTreeSet<_> = MANAGED_AGENT_METHODS.iter().copied().collect();
    assert!(allowed.is_subset(&registered));

    for method in registered.difference(&allowed) {
        let error = child
            .send_request(method, serde_json::Value::Null)
            .await
            .expect_err("unlisted registered method must be denied");
        if method.starts_with("hub/") || method.starts_with("meta/") {
            assert_central_denial(method, error);
        } else {
            assert_eq!(
                error.remote_code(),
                Some(jsonrpc::METHOD_NOT_FOUND),
                "{method} must not enter the Hub dispatcher from Agent IO"
            );
        }
    }

    let locked = hub.lock().await;
    assert!(locked.registry.get_agent_connection("child").is_some());
    assert!(locked.pending_permissions.is_empty());
    assert!(locked.pending_questions.is_empty());
    assert!(locked.pending_plan_approvals.is_empty());
    assert!(locked.uplink.is_none());
}

#[tokio::test]
async fn unknown_and_disabled_privileged_methods_fail_before_fallback() {
    let (_hub, child, _child_rx) = managed_child().await;
    for method in [
        "hub/future_admin",
        "hub/workflow/start",
        "meta/future_admin",
        "meta/spawn",
    ] {
        let error = child
            .send_request(method, serde_json::json!({"malicious": true}))
            .await
            .expect_err("unknown privileged method must be denied");
        assert_central_denial(method, error);
    }
}
