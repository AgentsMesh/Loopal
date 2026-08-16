use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptState, WorkflowEventPayload,
    WorkflowExecution, WorkflowFailureClass, WorkflowLimits, WorkflowOutputContract,
    WorkflowPlanDecision, WorkflowRunState, WorkflowSpec, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubHarness, replay_workflow, workflow_events};

#[tokio::test]
async fn run_deadline_interrupts_a_real_running_worker() {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow {
            spec: deadline_spec(),
        },
    })
    .unwrap();
    let mut harness = HubHarness::start_with_workflow(json!({
        "version": 3,
        "name": "production_workflow_run_deadline",
        "calls": [
            {"label": "planner", "expect": {"userContains": "run-deadline-e2e"},
             "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
            {"label": "deadline-worker", "expect": {"userContains": "deadline-worker-canary"},
             "chunks": [
                {"type": "text", "text": "partial-before-deadline"},
                {"type": "delay", "ms": 30_000},
                {"type": "text", "text": "must-not-complete"},
                {"type": "done"}
             ]}
        ]
    }))
    .await;

    let outcome = harness
        .workflow_turn(
            "Run run-deadline-e2e with multiple agents independently and cross-check it.",
        )
        .await;
    assert!(
        outcome.error.is_none(),
        "workflow input failed: {outcome:?}"
    );
    let terminal = outcome.summaries.last().expect("terminal workflow summary");
    assert_eq!(terminal.state, WorkflowRunState::Failed);

    let replay = harness.workflow_replay(&terminal.id);
    let events = workflow_events(&replay);
    let stop = events
        .iter()
        .position(|event| {
            matches!(
                &event.payload,
                WorkflowEventPayload::AttemptStopRequested { .. }
            )
        })
        .expect("deadline must durably request an exact worker stop");
    let failed = events
        .iter()
        .position(|event| matches!(&event.payload, WorkflowEventPayload::AttemptFailed { .. }))
        .expect("deadline must durably terminalize the attempt");
    assert!(stop < failed, "events: {events:?}");

    let run = replay_workflow(replay);
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.failure.unwrap().class, WorkflowFailureClass::Permanent);
    assert_eq!(run.attempts.len(), 1);
    let attempt = &run.attempts[0];
    assert_eq!(attempt.state, WorkflowAttemptState::Failed);
    assert!(
        attempt.entered_running,
        "deadline fired before real agent/start: {attempt:?}"
    );
    assert!(
        attempt.agent.is_some(),
        "deadline worker was never bound: {attempt:?}"
    );
    assert_eq!(
        attempt.failure.as_ref().map(|failure| failure.class),
        Some(WorkflowFailureClass::Permanent)
    );

    let requests = harness.journal().await;
    assert_eq!(requests.as_array().map(Vec::len), Some(2), "{requests}");
    assert_eq!(requests[1]["callLabel"], "deadline-worker");
}

fn deadline_spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "production run deadline".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "deadline-worker-canary".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 5_000,
            attempt_timeout_ms: 5_000,
            max_output_bytes: 4_096,
        },
        output_node: "worker".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}
