use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowExecution, WorkflowLimits, WorkflowOutput,
    WorkflowOutputContract, WorkflowPlanDecision, WorkflowRunState, WorkflowSpec,
    WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubHarness, replay_workflow};

#[tokio::test]
async fn terminal_workflow_survives_real_hub_resume_without_reexecution() {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: spec() },
    })
    .unwrap();
    let mut harness = HubHarness::start_with_workflow(json!({
        "version": 3,
        "name": "production_workflow_before_restart",
        "calls": [
            {"label": "planner", "expect": {"userContains": "restart-recovery-e2e"},
             "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
            {"label": "worker", "expect": {"userContains": "restart-worker-canary"},
             "chunks": [{"type": "text", "text": "durable-restart-output"},
                        {"type": "done"}]}
        ]
    }))
    .await;

    let outcome = harness
        .workflow_turn(
            "Run restart-recovery-e2e with multiple agents independently and cross-check it.",
        )
        .await;
    assert!(
        outcome.error.is_none(),
        "workflow input failed: {outcome:?}"
    );
    let terminal = outcome.summaries.last().expect("terminal workflow summary");
    assert_eq!(terminal.state, WorkflowRunState::Succeeded);
    let run_before = replay_workflow(harness.workflow_replay(&terminal.id));
    let session_id = harness.session_id.clone();

    let resumed = harness
        .restart_with_resume(json!({
            "version": 3,
            "name": "production_workflow_after_restart",
            "calls": []
        }))
        .await;
    assert_eq!(resumed.session_id, session_id);

    let snapshot = resumed.root_view_snapshot().await;
    let recent = snapshot["state"]["workflows"]["recent"]
        .as_array()
        .expect("root view exposes recovered terminal workflows");
    let recovered = recent
        .iter()
        .find(|summary| summary["id"] == terminal.id.as_str())
        .expect("terminal workflow missing from resumed projection");
    assert_eq!(recovered["state"], "succeeded");

    let run_after = replay_workflow(resumed.workflow_replay(&terminal.id));
    assert_eq!(run_after, run_before);
    assert_eq!(
        run_after.result,
        Some(WorkflowOutput::Text("durable-restart-output".into()))
    );
    assert_eq!(run_after.attempts.len(), 1);
    assert_eq!(resumed.journal().await, json!([]));
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "production restart recovery".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "restart-worker-canary".into(),
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
