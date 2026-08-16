use loopal_protocol::{
    QualifiedAddress, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};

use crate::workflow::WorkflowOwner;
use crate::workflow::scheduler::WorkflowSpawnRequest;
use crate::workflow::worker_profile::ResolvedWorkflowWorkerProfile;

pub(super) fn causation(run: &str, node: &str, attempt: &str) -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new(run),
        node_id: WorkflowNodeId::new(node),
        attempt_id: WorkflowAttemptId::new(attempt),
    }
}

pub(super) fn request(causation: WorkflowPermissionCausation) -> WorkflowSpawnRequest {
    WorkflowSpawnRequest {
        owner: WorkflowOwner::new("session", QualifiedAddress::local("root")),
        causation,
        run_goal: "goal".into(),
        task: "task".into(),
        dependency_results: Vec::new(),
        worker_profile: ResolvedWorkflowWorkerProfile::Default,
        output_contract: None,
        completion_result_limit: 1_024,
        attempt_capability: loopal_protocol::WorkflowAttemptCapability::parse("22".repeat(32))
            .unwrap(),
    }
}
