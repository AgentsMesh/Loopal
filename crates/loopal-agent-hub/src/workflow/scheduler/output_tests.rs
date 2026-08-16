use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{
    AgentCompletion, QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptFailure,
    WorkflowAttemptId, WorkflowEventPayload, WorkflowFailureClass, WorkflowLimits, WorkflowNodeId,
    WorkflowOutput, WorkflowOutputContract, WorkflowRunId, WorkflowRunSnapshot, WorkflowSpec,
    WorkflowWorkerProfileRef,
};

use super::*;

include!("output_tests/outcomes.rs");

fn assert_rejected(prepared: PreparedOutcome) {
    assert!(matches!(
        prepared.payload,
        WorkflowEventPayload::AttemptFailed {
            completion: AgentCompletion { ref reason, .. },
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                ..
            },
            ..
        } if reason == REJECTED_REASON
    ));
}

fn run(id: &str) -> WorkflowRunSnapshot {
    WorkflowRunSnapshot::planned(
        WorkflowRunId::new(id),
        QualifiedAddress::local("root"),
        spec(),
        1,
    )
}

fn key(run: &WorkflowRunSnapshot, id: &str) -> AttemptKey {
    AttemptKey {
        run_id: run.id.clone(),
        node_id: WorkflowNodeId::new("source"),
        attempt_id: WorkflowAttemptId::new(id),
    }
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "redact worker output".into(),
        nodes: vec![WorkflowAgentNode {
            id: WorkflowNodeId::new("source"),
            dependencies: Vec::new(),
            task: "return output".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 4_096,
        },
        output_node: WorkflowNodeId::new("source"),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}
