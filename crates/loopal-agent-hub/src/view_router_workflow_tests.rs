use loopal_protocol::{AgentEventPayload, WorkflowRunSummary};
use loopal_view_state::ViewSnapshot;

use super::test_support::{
    TestJournal, coordinator, owner, recovered_run, request, root_hub, shutdown,
};
use super::{handle_snapshot, workflow_authority};

#[tokio::test]
async fn root_snapshot_merges_authority_and_preserves_projection_revision() {
    let (hub, _execution) = root_hub(Some("snapshot-session"));
    let journal = TestJournal::new();
    let (handle, task) = coordinator(journal);
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());
    let response = handle
        .start(owner("snapshot-session"), request("request-snapshot"))
        .await
        .unwrap();
    let view = hub
        .lock()
        .await
        .registry
        .agent_view(loopal_protocol::ROOT_AGENT_NAME)
        .unwrap();
    view.lock().await.apply(AgentEventPayload::ModeChanged {
        mode: "ultra".into(),
    });
    let expected_rev = view.lock().await.rev();

    let value = handle_snapshot(
        &hub,
        serde_json::json!({"agent": loopal_protocol::ROOT_AGENT_NAME}),
    )
    .await
    .unwrap();
    let snapshot: ViewSnapshot = serde_json::from_value(value).unwrap();
    assert_eq!(snapshot.rev, expected_rev);
    assert_eq!(snapshot.state.agent.observable.mode, "ultra");
    assert_eq!(snapshot.state.workflows.active, vec![response.summary]);
    assert_eq!(snapshot.state.workflows.recent.len(), 0);
    shutdown(&hub, handle, task).await;
}

#[tokio::test]
async fn recovery_snapshot_replaces_lost_projection() {
    let (hub, _execution) = root_hub(Some("recovery-session"));
    let journal = TestJournal::new();
    journal.push_recovery(crate::workflow::journal::RecoveredOwner {
        runs: vec![recovered_run("wrun_recovered")],
        requests: Default::default(),
        delivery_intents: Vec::new(),
        acked_deliveries: Default::default(),
    });
    let (handle, task) = coordinator(journal);
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());

    let value = handle_snapshot(
        &hub,
        serde_json::json!({"agent": loopal_protocol::ROOT_AGENT_NAME}),
    )
    .await
    .unwrap();
    let snapshot: ViewSnapshot = serde_json::from_value(value).unwrap();
    assert_eq!(snapshot.state.workflows.active.len(), 1);
    assert_eq!(
        snapshot.state.workflows.active[0].id.as_str(),
        "wrun_recovered"
    );
    shutdown(&hub, handle, task).await;
}

#[tokio::test]
async fn coordinator_failure_keeps_reducer_workflows() {
    let (hub, _execution) = root_hub(Some("failure-session"));
    let (handle, task) = crate::workflow::WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());
    let view = hub
        .lock()
        .await
        .registry
        .agent_view(loopal_protocol::ROOT_AGENT_NAME)
        .unwrap();
    let summary = WorkflowRunSummary::from(&recovered_run("wrun_projected"));
    view.lock()
        .await
        .apply(AgentEventPayload::WorkflowRunChanged(summary.clone()));

    let value = handle_snapshot(
        &hub,
        serde_json::json!({"agent": loopal_protocol::ROOT_AGENT_NAME}),
    )
    .await
    .unwrap();
    let snapshot: ViewSnapshot = serde_json::from_value(value).unwrap();
    assert_eq!(snapshot.state.workflows.active, vec![summary]);
    shutdown(&hub, handle, task).await;
}

#[tokio::test]
async fn child_and_unbound_root_have_no_workflow_authority() {
    let (hub, root_execution) = root_hub(Some("authority-session"));
    let child_execution = {
        let mut locked = hub.lock().await;
        let (_peer, transport) = loopal_ipc::duplex_pair();
        let child = locked
            .registry
            .register_connection_with_parent_execution(
                "child",
                loopal_ipc::Connection::new(transport).into_listening().0,
                Some(root_execution.address.clone()),
                None,
                None,
            )
            .unwrap();
        let mut facts = locked
            .registry
            .runtime_facts(&root_execution)
            .unwrap()
            .clone();
        facts.origin = crate::types::AgentOrigin::ManagedChild;
        facts.parent = Some(root_execution.clone());
        facts.depth = 1;
        assert!(locked.registry.set_runtime_facts(&child, facts));
        child
    };
    let locked = hub.lock().await;
    assert!(workflow_authority(&locked, "child").is_none());
    drop(locked);

    let (unbound, _execution) = root_hub(None);
    let unbound_guard = unbound.lock().await;
    assert!(workflow_authority(&unbound_guard, loopal_protocol::ROOT_AGENT_NAME).is_none());
    assert_eq!(child_execution.address.agent, "child");
}
