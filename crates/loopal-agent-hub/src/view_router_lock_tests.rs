use std::time::Duration;

use super::handle_snapshot;
use super::test_support::root_hub;

#[tokio::test]
async fn coordinator_query_does_not_hold_hub_or_reducer_locks() {
    let (hub, _execution) = root_hub(Some("lock-session"));
    let (handle, blocked_task, seen) =
        crate::workflow::WorkflowCoordinatorHandle::spawn_test_blocked();
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());
    let snapshot_task = tokio::spawn({
        let hub = hub.clone();
        async move {
            handle_snapshot(
                &hub,
                serde_json::json!({"agent": loopal_protocol::ROOT_AGENT_NAME}),
            )
            .await
        }
    });
    tokio::time::timeout(Duration::from_secs(1), seen)
        .await
        .expect("snapshot did not reach coordinator")
        .expect("blocked coordinator was dropped");

    let hub_guard = tokio::time::timeout(Duration::from_secs(1), hub.lock())
        .await
        .expect("Hub lock held across coordinator await");
    let view = hub_guard
        .registry
        .agent_view(loopal_protocol::ROOT_AGENT_NAME)
        .unwrap();
    drop(hub_guard);
    let _reducer_guard = tokio::time::timeout(Duration::from_secs(1), view.lock())
        .await
        .expect("reducer lock held across coordinator await");

    snapshot_task.abort();
    blocked_task.abort();
    hub.lock().await.clear_workflow_coordinator();
}
