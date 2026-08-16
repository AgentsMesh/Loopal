use super::handle_control;
use super::interrupt_tests::fixture;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

#[tokio::test]
async fn workflow_enabled_managed_root_rejects_session_hot_swap_before_forwarding() {
    let (hub, _transport, mut sent) = fixture(false).await;
    let execution = {
        let mut locked = hub.lock().await;
        let execution = locked.registry.current_execution("main").unwrap();
        assert!(locked.registry.set_runtime_facts(
            &execution,
            AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default()),
        ));
        execution
    };
    assert_eq!(execution.address.agent, "main");
    let (coordinator, task) = crate::workflow::WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(coordinator.clone());

    let error = handle_control(
        &hub,
        serde_json::json!({
            "target": "main",
            "command": {"ResumeSession": "session-two"}
        }),
    )
    .await
    .unwrap_err();
    assert!(error.contains("hot-swap"));
    assert!(sent.try_recv().is_err());

    hub.lock().await.clear_workflow_coordinator();
    coordinator.shutdown().await.unwrap();
    task.await.unwrap();
}

#[tokio::test]
async fn workflow_enabled_non_resume_control_is_still_forwarded() {
    let (hub, transport, mut sent) = fixture(false).await;
    {
        let mut locked = hub.lock().await;
        let execution = locked.registry.current_execution("main").unwrap();
        assert!(locked.registry.set_runtime_facts(
            &execution,
            AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default()),
        ));
    }
    let (coordinator, task) = crate::workflow::WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(coordinator.clone());

    let request = tokio::spawn({
        let hub = hub.clone();
        async move {
            handle_control(
                &hub,
                serde_json::json!({"target": "main", "command": "Clear"}),
            )
            .await
        }
    });
    let wire = sent.recv().await.unwrap();
    let wire: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    transport
        .incoming_tx
        .send(loopal_ipc::jsonrpc::encode_response(
            wire["id"].as_i64().unwrap(),
            serde_json::json!({"status": "applied"}),
        ))
        .unwrap();
    assert_eq!(
        request.await.unwrap().unwrap(),
        serde_json::json!({"status": "applied"})
    );

    hub.lock().await.clear_workflow_coordinator();
    coordinator.shutdown().await.unwrap();
    task.await.unwrap();
}
