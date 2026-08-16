use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptState, WorkflowEventPayload,
    WorkflowExecution, WorkflowFailureClass, WorkflowLimits, WorkflowOutputContract,
    WorkflowPlanDecision, WorkflowRunState, WorkflowSpec, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubEnv, HubHarness, replay_workflow, workflow_events};

#[tokio::test]
async fn running_workflow_becomes_ambiguous_after_real_hub_crash() {
    let env = HubEnv::new();
    let marker = env.cwd.path().join("crash-effect-marker");
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: crash_spec() },
    })
    .unwrap();
    let scenario = json!({
        "version": 3,
        "name": "production_workflow_crash_recovery",
        "calls": [
            {"label": "planner", "expect": {"userContains": "crash-recovery-e2e"},
             "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
            {"label": "crash-worker-effect", "expect": {"userContains": "crash-worker-canary"},
             "chunks": [
                {"type": "tool_use", "id": "crash-effect-1", "name": "Bash",
                 "input": {
                    "command": "printf '%s\\n' crash-effect >> \"$CRASH_EFFECT_MARKER\"",
                    "env": {"CRASH_EFFECT_MARKER": marker.to_string_lossy()}
                 }},
                {"type": "done"}
             ]},
            {"label": "crash-worker-stall", "expect": {"toolResultId": "crash-effect-1"},
             "chunks": [
                {"type": "text", "text": "worker-running-after-effect"},
                {"type": "delay", "ms": 30_000},
                {"type": "text", "text": "must-not-complete"},
                {"type": "done"}
             ]}
        ]
    });
    let mut harness = HubHarness::start_with_workflow_env(env, scenario).await;

    let running = harness
        .start_workflow_until_stream(
            "Run crash-recovery-e2e with multiple agents independently and cross-check it.",
            "worker-running-after-effect",
            Some("crash-effect-1"),
        )
        .await;
    let before = replay_workflow(harness.workflow_replay(&running.id));
    assert_eq!(before.state, WorkflowRunState::Running);
    assert_eq!(before.attempts.len(), 1);
    let attempt_before = &before.attempts[0];
    assert_eq!(attempt_before.state, WorkflowAttemptState::Running);
    assert!(attempt_before.entered_running);
    assert!(attempt_before.agent.is_some());
    let attempt_id = attempt_before.id.clone();
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "crash-effect\n");
    let calls_before = harness.journal().await;
    let labels: Vec<_> = calls_before
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["callLabel"].as_str().unwrap())
        .collect();
    assert_eq!(
        labels,
        ["planner", "crash-worker-effect", "crash-worker-stall"]
    );
    assert!(
        calls_before
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["matched"] == true)
    );

    let mut resumed = harness
        .crash_with_resume(json!({
            "version": 3,
            "name": "production_workflow_after_crash",
            "calls": []
        }))
        .await;
    let terminal = resumed.wait_for_workflow_terminal(&running.id).await;
    assert_eq!(terminal.state, WorkflowRunState::Failed);

    let replay = resumed.workflow_replay(&running.id);
    let events = workflow_events(&replay);
    let entered_running = events
        .iter()
        .position(|event| {
            matches!(&event.payload,
                WorkflowEventPayload::AttemptRunning { attempt_id: running_id, .. }
                    if running_id == &attempt_id)
        })
        .expect("durable running attempt before crash");
    let failed = events
        .iter()
        .position(|event| {
            matches!(&event.payload,
                WorkflowEventPayload::AttemptFailed {
                    attempt_id: failed_id, completion, failure, ..
                } if failed_id == &attempt_id
                    && completion.reason == "workflow_recovery_unreconciled"
                    && failure.class == WorkflowFailureClass::AmbiguousExecution)
        })
        .expect("durable ambiguous failure after recovery grace");
    assert!(entered_running < failed, "events: {events:?}");

    let run = replay_workflow(replay);
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(
        run.failure.unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    assert_eq!(
        run.attempts.len(),
        1,
        "ambiguity must not retry: {:?}",
        run.attempts
    );
    let attempt = &run.attempts[0];
    assert_eq!(attempt.id, attempt_id);
    assert_eq!(attempt.state, WorkflowAttemptState::Failed);
    assert!(attempt.entered_running);
    assert!(attempt.agent.is_some());
    assert_eq!(
        attempt.failure.as_ref().map(|failure| failure.class),
        Some(WorkflowFailureClass::AmbiguousExecution)
    );
    assert_eq!(resumed.journal().await, json!([]));
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "crash-effect\n");
}

fn crash_spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "production crash recovery".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "crash-worker-canary".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 2,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 4_096,
        },
        output_node: "worker".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}
