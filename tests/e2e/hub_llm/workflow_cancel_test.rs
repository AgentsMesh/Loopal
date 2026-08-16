use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptState, WorkflowEventPayload,
    WorkflowExecution, WorkflowLimits, WorkflowOutputContract, WorkflowPlanDecision,
    WorkflowRunState, WorkflowSpec, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubHarness, replay_workflow, workflow_events};

#[tokio::test]
async fn root_cancels_a_real_running_workflow_worker() {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow {
            spec: cancel_spec(),
        },
    })
    .unwrap();
    let mut harness = HubHarness::start_with_workflow(json!({
        "version": 3,
        "name": "production_workflow_cancel",
        "calls": [
            {"label": "planner", "expect": {"userContains": "cancel-running-e2e"},
             "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
            {"label": "cancel-worker", "expect": {"userContains": "cancel-worker-canary"},
             "chunks": [
                {"type": "text", "text": "worker-running-before-cancel"},
                {"type": "delay", "ms": 30_000},
                {"type": "text", "text": "must-not-complete"},
                {"type": "done"}
             ]}
        ]
    }))
    .await;

    let running = harness
        .start_workflow_until_stream(
            "Run cancel-running-e2e with multiple agents independently and cross-check it.",
            "worker-running-before-cancel",
            None,
        )
        .await;
    let cancel_text = format!("Cancel workflow {} now.", running.id);
    loopal_mock_llm_server::append_mock_calls(
        &harness.base_url,
        vec![
            json!({
                "label": "cancel-root",
                "expect": {"userContains": cancel_text},
                "chunks": [
                    {"type": "tool_use", "id": "cancel-1", "name": "workflow_cancel",
                     "input": {
                        "request_id": "wreq_user_cancel_e2e",
                        "run_id": running.id.as_str(),
                        "reason": "user requested cancellation"
                     }},
                    {"type": "done"}
                ]
            }),
            json!({
                "label": "cancel-continuation",
                "expect": {"toolResultId": "cancel-1"},
                "chunks": [
                    {"type": "text", "text": "cancel requested"},
                    {"type": "done"}
                ]
            }),
            json!({
                "label": "cancel-terminal-root",
                "expect": {"userContains": "was cancelled"},
                "chunks": [
                    {"type": "text", "text": "cancel-terminal-consumed"},
                    {"type": "done"}
                ]
            }),
        ],
    )
    .await;

    let terminal = harness.cancel_workflow_turn(&running.id).await;
    assert_eq!(terminal.state, WorkflowRunState::Cancelled);
    assert_eq!(terminal.counts.cancelled, 1);
    harness
        .wait_for_terminal_root_response("cancel-terminal-consumed")
        .await;
    let delivery = harness.wait_for_delivery_ack(&running.id).await;
    let delivered = harness.persisted_workflow_results(&running.id);
    assert_eq!(delivered.len(), 1, "workflow result turns: {delivered:?}");
    assert_eq!(delivered[0].state, "cancelled");
    assert_eq!(delivered[0].terminal_revision, terminal.revision);
    assert!(delivered[0].content.contains("was cancelled"));
    assert_eq!(delivery.terminal_revision, terminal.revision);

    let replay = harness.workflow_replay(&running.id);
    let events = workflow_events(&replay);
    let requested = events
        .iter()
        .position(|event| matches!(&event.payload, WorkflowEventPayload::CancelRequested { .. }))
        .expect("durable cancel request");
    let cancelled = events
        .iter()
        .position(|event| {
            matches!(
                &event.payload,
                WorkflowEventPayload::AttemptCancelled { .. }
            )
        })
        .expect("durable attempt cancellation");
    assert!(requested < cancelled, "events: {events:?}");

    let run = replay_workflow(replay);
    assert_eq!(run.state, WorkflowRunState::Cancelled);
    assert_eq!(run.attempts.len(), 1);
    let attempt = &run.attempts[0];
    assert_eq!(attempt.state, WorkflowAttemptState::Cancelled);
    assert!(
        attempt.entered_running,
        "cancel raced real agent/start: {attempt:?}"
    );
    assert!(
        attempt.agent.is_some(),
        "cancelled worker had no real lease: {attempt:?}"
    );

    let requests = harness.journal().await;
    let labels: Vec<_> = requests
        .as_array()
        .unwrap()
        .iter()
        .map(|request| request["callLabel"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            "planner",
            "cancel-worker",
            "cancel-root",
            "cancel-continuation",
            "cancel-terminal-root"
        ]
    );
    assert!(
        requests
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["matched"] == true)
    );
}

fn cancel_spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "production workflow cancellation".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "cancel-worker-canary".into(),
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
