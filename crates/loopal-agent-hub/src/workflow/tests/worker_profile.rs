use std::sync::Arc;

use loopal_protocol::{WorkflowAttemptId, WorkflowNodeId, WorkflowRunId, WorkflowWorkerProfileRef};

use super::journal_support::TestJournal;
use super::scheduler_support::{coordinator, test_spawner};
use super::support::{owner, request};
use crate::workflow::WorkflowCoordinatorError;
use crate::workflow::worker_profile::ResolvedWorkflowWorkerProfile;

#[test]
fn registry_resolves_only_builtin_v1_profiles() {
    for (name, expected) in [
        ("default", ResolvedWorkflowWorkerProfile::Default),
        ("explore", ResolvedWorkflowWorkerProfile::Explore),
        ("plan", ResolvedWorkflowWorkerProfile::Plan),
    ] {
        let resolved =
            ResolvedWorkflowWorkerProfile::resolve(&WorkflowWorkerProfileRef::new(name)).unwrap();
        assert_eq!(resolved, expected);
        assert_eq!(resolved.agent_type(), name);
    }

    assert!(matches!(
        ResolvedWorkflowWorkerProfile::resolve(&WorkflowWorkerProfileRef::new("general")),
        Err(WorkflowCoordinatorError::UnsupportedWorkerProfile { profile })
            if profile.as_str() == "general"
    ));
}

#[tokio::test]
async fn unknown_profile_is_rejected_before_durable_or_spawn_effects() {
    let journal = Arc::new(TestJournal::new());
    let (spawner, control) = test_spawner();
    let (handle, task, clock, ids) = coordinator(
        [],
        [WorkflowRunId::new("wrun_unused")],
        [WorkflowAttemptId::new("watt_unused")],
        journal.clone(),
        spawner,
    );
    let mut start = request("wreq_unknown_profile");
    start.spec.nodes[0].worker_profile = WorkflowWorkerProfileRef::new("custom");

    assert_eq!(
        handle.start(owner("session", "root"), start).await,
        Err(WorkflowCoordinatorError::UnsupportedWorkerProfileForNode {
            node_id: WorkflowNodeId::new("source"),
            profile: WorkflowWorkerProfileRef::new("custom"),
        })
    );
    assert_eq!(clock.calls(), 0);
    assert_eq!(ids.calls(), 0);
    assert_eq!(ids.attempt_calls(), 0);
    assert!(journal.starts().is_empty());
    assert!(journal.events().is_empty());
    control.assert_idle().await;

    drop(handle);
    task.await.unwrap();
}
