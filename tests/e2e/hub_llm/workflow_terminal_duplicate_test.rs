use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowExecution, WorkflowLimits, WorkflowOutputContract,
    WorkflowPlanDecision, WorkflowRunState, WorkflowSpec, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::HubHarness;

#[tokio::test]
async fn redelivered_applied_identity_does_not_create_a_second_root_turn() {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: spec() },
    })
    .unwrap();
    let mut harness = HubHarness::start_with_workflow(json!({
        "version": 3, "name": "terminal_duplicate_before_restart", "calls": [
            {"label": "planner", "expect": {"userContains": "terminal-duplicate-goal"},
             "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
            {"label": "worker", "expect": {"userContains": "duplicate-worker-canary"},
             "chunks": [{"type": "text", "text": "duplicate-final-result"},
                        {"type": "done"}]},
            {"label": "terminal-root", "expect": {"userContains": "duplicate-final-result"},
             "chunks": [{"type": "text", "text": "duplicate-first-consumed"},
                        {"type": "done"}]}
        ]
    }))
    .await;

    let outcome = harness
        .workflow_turn("Run terminal-duplicate-goal with independent workers and cross-check it.")
        .await;
    let terminal = outcome.summaries.last().expect("terminal summary").clone();
    assert_eq!(terminal.state, WorkflowRunState::Succeeded);
    harness
        .wait_for_terminal_root_response("duplicate-first-consumed")
        .await;
    let original = harness.wait_for_delivery_ack(&terminal.id).await;
    let before = harness.persisted_workflow_results(&terminal.id);
    assert_eq!(before.len(), 1, "workflow result turns: {before:?}");
    assert!(before[0].payload_digest.starts_with("sha256:"));

    let resumed = harness
        .restart_with_missing_delivery_ack(
            &terminal.id,
            json!({"version": 3, "name": "terminal_duplicate_after_restart", "calls": []}),
        )
        .await;
    let recovered = resumed.wait_for_delivery_ack(&terminal.id).await;
    assert_eq!(recovered, original);
    assert_eq!(resumed.persisted_workflow_results(&terminal.id), before);
    assert_eq!(resumed.journal().await, json!([]));
    assert_eq!(
        resumed.workflow_replay(&terminal.id).delivery_acks,
        vec![original]
    );
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "terminal-duplicate-goal".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "duplicate-worker-canary".into(),
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
        output_node: "worker".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}
