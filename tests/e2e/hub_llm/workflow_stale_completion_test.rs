use std::os::unix::fs::PermissionsExt;

use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptState, WorkflowEventPayload,
    WorkflowExecution, WorkflowFailureClass, WorkflowLimits, WorkflowOutputContract,
    WorkflowPlanDecision, WorkflowRunState, WorkflowSpec, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubEnv, HubHarness, replay_workflow, workflow_events};

const LATE_CANARY: &str = "late-success-must-not-win";

#[tokio::test]
async fn success_reported_after_timeout_interrupt_cannot_win() {
    let mut env = HubEnv::new();
    let trace = env.home.path().join(".loopal/e2e-late-worker-trace");
    install_late_completion_worker(&mut env);
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: spec() },
    })
    .unwrap();
    let mut harness = HubHarness::start_with_workflow_env(
        env,
        json!({
            "version": 3,
            "name": "workflow_late_completion",
            "calls": [
                {"label": "planner", "expect": {"userContains": "late-completion-e2e"},
                 "chunks": [{"type": "text", "text": plan}, {"type": "done"}]}
            ]
        }),
    )
    .await;

    let outcome = harness
        .workflow_turn("Run late-completion-e2e with an independent worker and cross-check it.")
        .await;
    assert!(
        outcome.error.is_none(),
        "workflow input failed: {outcome:?}"
    );
    let terminal = outcome.summaries.last().expect("terminal summary");
    assert_eq!(terminal.state, WorkflowRunState::Failed);

    let trace = std::fs::read_to_string(trace).expect("late worker protocol trace");
    assert_eq!(
        trace.lines().collect::<Vec<_>>(),
        [
            "initialize",
            "handshake",
            "started",
            "interrupt",
            "late_completion"
        ]
    );
    let replay = harness.workflow_replay(&terminal.id);
    let replay_wire = format!("{replay:?}");
    let events = workflow_events(&replay);
    let stop = events
        .iter()
        .position(|event| {
            matches!(
                &event.payload,
                WorkflowEventPayload::AttemptStopRequested { .. }
            )
        })
        .expect("timeout stop request");
    let failed = events
        .iter()
        .position(|event| matches!(&event.payload, WorkflowEventPayload::AttemptFailed { .. }))
        .expect("timeout attempt failure");
    assert!(stop < failed, "events: {events:?}");
    assert!(!events.iter().any(|event| {
        matches!(
            &event.payload,
            WorkflowEventPayload::AttemptSucceeded { .. }
        )
    }));

    let run = replay_workflow(replay);
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.revision, terminal.revision);
    assert_eq!(run.result, None);
    assert_eq!(
        run.failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );
    assert_eq!(run.attempts.len(), 1);
    let attempt = &run.attempts[0];
    assert_eq!(attempt.state, WorkflowAttemptState::Failed);
    assert!(attempt.entered_running);
    assert_eq!(
        attempt.completion.as_ref().unwrap().reason,
        "workflow_timeout"
    );
    assert_eq!(attempt.completion.as_ref().unwrap().result, None);
    assert_eq!(
        attempt.failure.as_ref().unwrap().class,
        WorkflowFailureClass::AmbiguousExecution
    );

    let replayed_again = replay_workflow(harness.workflow_replay(&terminal.id));
    assert_eq!(
        replayed_again, run,
        "terminal replay changed after late completion"
    );
    let journal = harness.journal().await;
    assert_eq!(journal.as_array().map(Vec::len), Some(1), "{journal}");
    assert_eq!(journal[0]["callLabel"], "planner");
    for sink in [outcome.events.join("\n"), replay_wire, format!("{run:?}")] {
        assert!(
            !sink.contains(LATE_CANARY),
            "late success leaked into state: {sink}"
        );
    }
}

fn install_late_completion_worker(env: &mut HubEnv) {
    let fixture = loopal_agent_client::require_runfile_env("LOOPAL_MOCK_WORKFLOW_WORKER_BINARY")
        .expect("resolve LOOPAL_MOCK_WORKFLOW_WORKER_BINARY");
    let path = env.home.path().join("workflow-late-completion-shim");
    let script = format!(
        r#"#!/bin/sh
state="$HOME/.loopal/e2e-late-agent-launches"
mkdir -p "$HOME/.loopal"
if mkdir "${{state}}-root" 2>/dev/null; then
  exec "$LOOPAL_E2E_REAL_BINARY" "$@"
fi
export LOOPAL_E2E_WORKER_TRACE="$HOME/.loopal/e2e-late-worker-trace"
exec {} "$@"
"#,
        shell_quote(&fixture.to_string_lossy())
    );
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    env.agent_binary_override = Some(path);
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "late completion containment".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "fixture worker waits for timeout interrupt".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 30_000,
            // The attempt deadline starts at dispatch reservation and includes
            // real process startup, initialize, binding, and agent/start.
            attempt_timeout_ms: 10_000,
            max_output_bytes: 4_096,
        },
        output_node: "worker".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}
