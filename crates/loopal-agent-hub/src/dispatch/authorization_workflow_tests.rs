use std::sync::Arc;

use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use tokio::sync::{Mutex, mpsc};

use super::authorization;
use crate::Hub;
use crate::request_principal::{AgentPrincipal, HubRequestPrincipal};
use crate::types::{AgentOrigin, AgentRuntimeFacts, SpawnAuthority};

fn connection() -> Arc<Connection<loopal_ipc::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

#[tokio::test]
async fn workflow_acl_requires_installed_backend_and_authenticated_root() {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("main", connection(), None, None, None)
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = Some("session-workflow".into());
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts.clone())
    );
    let root = || {
        Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
            execution.clone(),
            facts.clone(),
        )))
    };
    assert!(
        authorization::authorize(&hub, methods::HUB_WORKFLOW_START.name, root())
            .await
            .is_err()
    );

    let (coordinator, task) = crate::workflow::WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(coordinator.clone());
    for method in [
        methods::HUB_WORKFLOW_START.name,
        methods::HUB_WORKFLOW_LOOKUP_START.name,
        methods::HUB_WORKFLOW_GET.name,
        methods::HUB_WORKFLOW_WAIT.name,
        methods::HUB_WORKFLOW_CANCEL.name,
    ] {
        assert!(authorization::authorize(&hub, method, root()).await.is_ok());
    }

    let child_execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_exact_parent_execution(
            "child",
            connection(),
            Some(loopal_protocol::QualifiedAddress::local("main")),
            Some(&execution),
            None,
            None,
            true,
        )
        .unwrap();
    let mut child_facts = facts;
    child_facts.origin = AgentOrigin::ManagedChild;
    child_facts.parent = Some(execution);
    child_facts.depth = 1;
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&child_execution, child_facts.clone())
    );
    let child = Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
        child_execution,
        child_facts,
    )));
    assert!(
        authorization::authorize(&hub, methods::HUB_WORKFLOW_START.name, child)
            .await
            .is_err()
    );

    hub.lock().await.clear_workflow_coordinator();
    coordinator.shutdown().await.unwrap();
    task.await.unwrap();
}
