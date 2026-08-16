use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_ipc::{Connection, DispatcherBuilder, HandlerCtx};
use tokio::sync::{Mutex, mpsc};

use super::register;
use crate::Hub;
use crate::request_principal::{AgentPrincipal, HubRequestPrincipal};
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

async fn fixture() -> (
    Arc<Mutex<Hub>>,
    crate::types::AgentExecutionRef,
    HandlerCtx,
    tempfile::TempDir,
) {
    let temp = tempfile::tempdir().unwrap();
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::with_cwd(events, temp.path().into())));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let mut locked = hub.lock().await;
    let execution = locked
        .registry
        .register_connection_with_parent_execution("agent", connection, None, None, None)
        .unwrap();
    let facts = AgentRuntimeFacts::root(temp.path().into(), SpawnAuthority::default());
    assert!(locked.registry.set_runtime_facts(&execution, facts.clone()));
    assert!(
        locked
            .spawn_registry
            .register_exact(execution.clone(), temp.path().into(), None)
    );
    let principal = AgentPrincipal::new(execution.clone(), facts);
    drop(locked);
    let context =
        HandlerCtx::new("agent").with_extension(Arc::new(HubRequestPrincipal::Agent(principal)));
    (hub, execution, context, temp)
}

#[tokio::test]
async fn callbacks_are_registered_and_forward_handler_errors() {
    let (hub, _execution, context, temp) = fixture().await;
    let dispatcher = register(DispatcherBuilder::new(), hub).build();
    assert_eq!(
        dispatcher.registered_methods(),
        [
            methods::HUB_SECRET_GET.name,
            methods::HUB_SECRET_HEALTH.name,
            methods::HUB_SECRET_LIST_NAMES.name,
            methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name,
        ]
    );

    let get = serde_json::json!({
        "cwd": temp.path().display().to_string(),
        "name": "key",
        "caller": {"agent_name": "agent", "depth": 0}
    });
    let error = dispatcher
        .dispatch(methods::HUB_SECRET_GET.name, get, &context)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("vault service not initialized"));
    let cwd = serde_json::json!({"cwd": temp.path().display().to_string()});
    for method in [
        methods::HUB_SECRET_LIST_NAMES.name,
        methods::HUB_SECRET_HEALTH.name,
    ] {
        let error = dispatcher
            .dispatch(method, cwd.clone(), &context)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("vault service not initialized"));
    }
}

#[tokio::test]
async fn every_callback_revalidates_stale_agent() {
    let (hub, execution, context, _temp) = fixture().await;
    let dispatcher = register(DispatcherBuilder::new(), hub.clone()).build();
    assert!(hub.lock().await.registry.unregister_exact(&execution));

    for method in [
        methods::HUB_SECRET_GET.name,
        methods::HUB_SECRET_LIST_NAMES.name,
        methods::HUB_SECRET_HEALTH.name,
        methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name,
    ] {
        let error = dispatcher
            .dispatch(method, serde_json::Value::Null, &context)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("stale Agent connection"));
    }
}
