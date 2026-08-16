use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_ipc::{Connection, DispatcherBuilder, HandlerCtx};
use tokio::sync::{Mutex, mpsc};

use super::register;
use crate::Hub;
use crate::request_principal::{AgentPrincipal, HubRequestPrincipal};
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

async fn fixture() -> (Arc<Mutex<Hub>>, crate::types::AgentExecutionRef, HandlerCtx) {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let mut locked = hub.lock().await;
    let execution = locked
        .registry
        .register_connection_with_parent_execution("agent", connection, None, None, None)
        .unwrap();
    let facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    assert!(locked.registry.set_runtime_facts(&execution, facts.clone()));
    let principal = AgentPrincipal::new(execution.clone(), facts);
    drop(locked);
    let context =
        HandlerCtx::new("agent").with_extension(Arc::new(HubRequestPrincipal::Agent(principal)));
    (hub, execution, context)
}

#[tokio::test]
async fn callbacks_are_registered_and_run_for_active_agent() {
    let (hub, _execution, context) = fixture().await;
    let dispatcher = register(DispatcherBuilder::new(), hub).build();
    assert_eq!(
        dispatcher.registered_methods(),
        [
            methods::HUB_MCP_CALL_TOOL.name,
            methods::HUB_MCP_LIST_TOOLS.name,
            methods::HUB_MCP_RECONNECT.name,
            methods::HUB_MCP_SNAPSHOT.name,
        ]
    );

    let list = dispatcher
        .dispatch(
            methods::HUB_MCP_LIST_TOOLS.name,
            serde_json::Value::Null,
            &context,
        )
        .await
        .unwrap();
    assert!(list["tools"].as_array().unwrap().is_empty());
    let snapshot = dispatcher
        .dispatch(
            methods::HUB_MCP_SNAPSHOT.name,
            serde_json::Value::Null,
            &context,
        )
        .await
        .unwrap();
    assert!(snapshot["servers"].as_array().unwrap().is_empty());
    for (method, params, expected) in [
        (
            methods::HUB_MCP_CALL_TOOL.name,
            serde_json::Value::Null,
            "invalid call_tool params",
        ),
        (
            methods::HUB_MCP_RECONNECT.name,
            serde_json::Value::Null,
            "invalid reconnect params",
        ),
    ] {
        let error = dispatcher
            .dispatch(method, params, &context)
            .await
            .unwrap_err();
        assert!(error.to_string().contains(expected));
    }
}

#[tokio::test]
async fn every_callback_revalidates_stale_agent() {
    let (hub, execution, context) = fixture().await;
    let dispatcher = register(DispatcherBuilder::new(), hub.clone()).build();
    assert!(hub.lock().await.registry.unregister_exact(&execution));

    for method in [
        methods::HUB_MCP_LIST_TOOLS.name,
        methods::HUB_MCP_CALL_TOOL.name,
        methods::HUB_MCP_RECONNECT.name,
        methods::HUB_MCP_SNAPSHOT.name,
    ] {
        let error = dispatcher
            .dispatch(method, serde_json::Value::Null, &context)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stale Agent connection"));
    }
}
