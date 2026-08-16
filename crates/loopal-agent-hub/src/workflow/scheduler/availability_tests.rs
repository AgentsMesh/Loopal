use loopal_protocol::{
    WorkflowAttemptCapability, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};

use super::*;

fn causation() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_unavailable"),
        node_id: WorkflowNodeId::new("source"),
        attempt_id: WorkflowAttemptId::new("watt_unavailable"),
    }
}

fn execution() -> AgentExecutionRef {
    AgentExecutionRef::local("worker", 7)
}

#[tokio::test]
async fn unavailable_spawner_fails_closed_for_supported_cleanup_operations() {
    let spawner = UnavailableWorkflowSpawner;
    let causation = causation();
    let request = WorkflowSpawnRequest {
        owner: WorkflowOwner::new("session", loopal_protocol::QualifiedAddress::local("root")),
        causation: causation.clone(),
        run_goal: "finish the run".into(),
        task: "complete source".into(),
        dependency_results: Vec::new(),
        worker_profile: ResolvedWorkflowWorkerProfile::Default,
        output_contract: None,
        completion_result_limit: 1_024,
        attempt_capability: WorkflowAttemptCapability::parse("11".repeat(32)).unwrap(),
    };

    let failure = match spawner.prepare(request).await {
        Err(failure) => failure,
        Ok(_) => panic!("unavailable spawner unexpectedly prepared a worker"),
    };
    assert_eq!(failure.completion.reason, "workflow_spawner_unavailable");
    assert_eq!(
        spawner
            .abort_prepare_and_wait(&causation, Duration::from_millis(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
    assert!(matches!(
        spawner
            .adopt_recovered(WorkflowRecoveryAdoptionRequest {
                owner: WorkflowOwner::new(
                    "session",
                    loopal_protocol::QualifiedAddress::local("root"),
                ),
                causation,
                execution: execution(),
                output_contract: None,
            })
            .await,
        Err(WorkflowRecoveryAdoptionError::MissingCustody)
    ));
    let activation =
        tokio::spawn(async { UnavailableWorkflowSpawner.activate(&execution()).await }).await;
    assert!(matches!(activation, Err(error) if error.is_panic()));
    assert_eq!(
        spawner.interrupt(&execution()).await,
        WorkflowStopStatus::Stopped
    );
    assert_eq!(
        spawner
            .shutdown_and_wait(&execution(), Duration::from_millis(1))
            .await,
        WorkflowCleanupStatus::Confirmed
    );
}
