use std::os::unix::fs::PermissionsExt;

use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowAttemptState, WorkflowExecution,
    WorkflowFailureClass, WorkflowLimits, WorkflowOutput, WorkflowOutputContract,
    WorkflowPlanDecision, WorkflowRunState, WorkflowSpec, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubEnv, HubHarness, replay_workflow};

#[tokio::test]
async fn preparation_failure_retries_with_a_second_real_worker_handshake() {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: retry_spec() },
    })
    .unwrap();
    let mut env = HubEnv::new();
    install_fail_first_workflow_child_shim(&mut env);
    let mut harness = HubHarness::start_with_workflow_env(
        env,
        json!({
            "version": 3,
            "name": "production_workflow_pre_execution_retry",
            "calls": [
                {"label": "planner", "expect": {"userContains": "retry-handshake-e2e"},
                 "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
                {"label": "retry-worker", "expect": {"userContains": "retry-worker-canary"},
                 "chunks": [
                    {"type": "text", "text": "retry-terminal-output"},
                    {"type": "done"}
                 ]}
            ]
        }),
    )
    .await;

    let outcome = harness
        .workflow_turn(
            "Run retry-handshake-e2e with multiple agents independently and cross-check it.",
        )
        .await;
    assert!(
        outcome.error.is_none(),
        "workflow input failed: {outcome:?}"
    );
    let terminal = outcome.summaries.last().expect("terminal workflow summary");
    assert_eq!(terminal.state, WorkflowRunState::Succeeded);

    let run = replay_workflow(harness.workflow_replay(&terminal.id));
    assert_eq!(
        run.result,
        Some(WorkflowOutput::Text("retry-terminal-output".into()))
    );
    assert_eq!(run.attempts.len(), 2, "attempts: {:?}", run.attempts);
    let first = &run.attempts[0];
    assert_eq!(first.state, WorkflowAttemptState::Failed);
    assert!(!first.entered_running);
    assert!(
        first.agent.is_none(),
        "failed preparation acquired a lease: {first:?}"
    );
    assert_eq!(
        first.failure.as_ref().map(|failure| failure.class),
        Some(WorkflowFailureClass::TransientBeforeExecution)
    );
    let retry = &run.attempts[1];
    assert_eq!(retry.state, WorkflowAttemptState::Succeeded);
    assert!(
        retry.entered_running,
        "retry never completed agent/start: {retry:?}"
    );
    assert_eq!(
        retry.agent.as_ref().map(|agent| agent.agent.clone()),
        Some(format!("workflow-{}", retry.id))
    );

    let requests = harness.journal().await;
    assert_eq!(requests.as_array().map(Vec::len), Some(2), "{requests}");
    assert_eq!(requests[0]["callLabel"], "planner");
    assert_eq!(requests[1]["callLabel"], "retry-worker");
    assert!(
        requests
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["matched"] == true)
    );
}

#[tokio::test]
async fn preparation_retry_exhaustion_fails_without_running_an_attempt() {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: retry_spec() },
    })
    .unwrap();
    let mut env = HubEnv::new();
    install_failing_workflow_children_shim(&mut env, 2);
    let mut harness = HubHarness::start_with_workflow_env(
        env,
        json!({
            "version": 3,
            "name": "production_workflow_retry_exhaustion",
            "calls": [
                {"label": "planner", "expect": {"userContains": "retry-exhaustion-e2e"},
                 "chunks": [{"type": "text", "text": plan}, {"type": "done"}]}
            ]
        }),
    )
    .await;

    let outcome = harness
        .workflow_turn(
            "Run retry-exhaustion-e2e with multiple agents independently and cross-check it.",
        )
        .await;
    assert!(
        outcome.error.is_none(),
        "workflow input failed: {outcome:?}"
    );
    let terminal = outcome.summaries.last().expect("terminal workflow summary");
    assert_eq!(terminal.state, WorkflowRunState::Failed);

    let run = replay_workflow(harness.workflow_replay(&terminal.id));
    assert_eq!(run.attempts.len(), 2, "attempts: {:?}", run.attempts);
    assert!(run.attempts.iter().all(|attempt| {
        attempt.state == WorkflowAttemptState::Failed
            && !attempt.entered_running
            && attempt.agent.is_none()
            && attempt.failure.as_ref().is_some_and(|failure| {
                failure.class == WorkflowFailureClass::TransientBeforeExecution
            })
    }));
    assert_eq!(
        run.failure.unwrap().class,
        WorkflowFailureClass::TransientBeforeExecution
    );
    let requests = harness.journal().await;
    assert_eq!(requests.as_array().map(Vec::len), Some(1), "{requests}");
    assert_eq!(requests[0]["callLabel"], "planner");
}

fn retry_spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "production pre-execution retry".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "retry-worker-canary".into(),
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

fn install_fail_first_workflow_child_shim(env: &mut HubEnv) {
    install_failing_workflow_children_shim(env, 1);
}

fn install_failing_workflow_children_shim(env: &mut HubEnv, failures: u32) {
    let path = env.home.path().join("workflow-agent-shim");
    let script = format!(
        r#"#!/bin/sh
state="$HOME/.loopal/e2e-agent-launches"
mkdir -p "$HOME/.loopal"
if mkdir "${{state}}-root" 2>/dev/null; then
  exec "$LOOPAL_E2E_REAL_BINARY" "$@"
fi
i=1
while [ "$i" -le {failures} ]; do
  if mkdir "${{state}}-failed-worker-$i" 2>/dev/null; then
    exit 86
  fi
  i=$((i + 1))
done
exec "$LOOPAL_E2E_REAL_BINARY" "$@"
"#
    );
    std::fs::write(&path, script).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    env.agent_binary_override = Some(path);
}
