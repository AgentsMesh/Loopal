use std::os::unix::fs::PermissionsExt;

use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowExecution, WorkflowLimits, WorkflowOutputContract,
    WorkflowPlanDecision, WorkflowRunState, WorkflowSpec, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubEnv, HubHarness, replay_workflow};

#[tokio::test]
async fn queued_terminal_delivery_retries_after_real_hub_crash_with_same_identity() {
    let mut env = HubEnv::new();
    let worker_pid = install_pid_recording_child_shim(&mut env);
    let workflow = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: spec() },
    })
    .unwrap();
    let mut harness = HubHarness::start_with_workflow_env(
        env,
        json!({"version": 3, "name": "terminal_delivery_before_crash", "calls": [
            {"label": "planner", "expect": {"userContains": "terminal-crash-goal"},
             "chunks": [{"type": "text", "text": workflow}, {"type": "done"}]},
            {"label": "worker", "expect": {"userContains": "crash-delivery-worker"},
             "chunks": [{"type": "text", "text": "worker-running-before-crash"},
                        {"type": "delay", "ms": 30000}, {"type": "done"}]},
            {"label": "busy-root", "expect": {"userContains": "occupy-root-for-delivery"},
             "chunks": [{"type": "text", "text": "busy-root-started"},
                        {"type": "delay", "ms": 6000},
                        {"type": "text", "text": "queued-window-open"},
                        {"type": "delay", "ms": 30000}, {"type": "done"}]}
        ]}),
    )
    .await;

    let running = harness
        .start_workflow_until_stream(
            "Run terminal-crash-goal with independent workers and cross-check it.",
            "worker-running-before-crash",
            None,
        )
        .await;
    harness
        .route_human("occupy-root-for-delivery with a detailed direct response")
        .await;
    harness.wait_for_root_stream("busy-root-started").await;
    let calls = harness.wait_for_mock_calls(3).await;
    assert_eq!(calls[0]["callLabel"], "planner");
    assert_eq!(calls[0]["matched"], true);
    assert_eq!(calls[1]["callLabel"], "worker");
    assert_eq!(calls[1]["matched"], true);
    assert_eq!(calls[2]["callLabel"], "busy-root");
    assert_eq!(calls[2]["matched"], true);
    terminate(read_pid(&worker_pid));
    harness.wait_for_root_stream("queued-window-open").await;

    let terminal = harness.wait_for_workflow_terminal(&running.id).await;
    assert_eq!(terminal.state, WorkflowRunState::Failed);
    let snapshot = replay_workflow(harness.workflow_replay(&running.id));
    let failure = snapshot.failure.as_ref().expect("terminal failure");
    let expected = WorkflowTerminalDeliveryId::new(
        harness.session_id.clone(),
        running.id.clone(),
        terminal.revision,
    );
    let expected_digest = WorkflowTerminalNotification {
        delivery_id: expected.clone(),
        state: WorkflowRunState::Failed,
        run_goal: snapshot.spec.run_goal.clone(),
        outcome: WorkflowTerminalOutcome::Failed {
            class: failure.class,
            reason: failure.reason.clone(),
        },
        content: format!(
            "Workflow {} failed.\n\nGoal: {}\n\nReason:\n{}",
            snapshot.id, snapshot.spec.run_goal, failure.reason
        ),
    }
    .payload_digest();
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    let before = harness.workflow_replay(&running.id);
    assert!(before.delivery_acks.is_empty());
    assert!(harness.persisted_workflow_results(&running.id).is_empty());

    let resumed = harness
        .crash_with_resume(json!({
            "version": 3, "name": "terminal_delivery_after_crash", "calls": [
                {"label": "terminal-root", "expect": {"userContains": "failed."},
                 "chunks": [{"type": "text", "text": "crash-delivery-consumed"},
                            {"type": "done"}]}
            ]
        }))
        .await;
    let calls = resumed.wait_for_mock_calls(1).await;
    assert_eq!(calls[0]["callLabel"], "terminal-root");
    assert_eq!(calls[0]["matched"], true);
    assert_eq!(resumed.wait_for_delivery_ack(&running.id).await, expected);
    let results = resumed.persisted_workflow_results(&running.id);
    assert_eq!(results.len(), 1, "workflow result turns: {results:?}");
    assert_eq!(results[0].session_id, resumed.session_id);
    assert_eq!(results[0].run_id, running.id.as_str());
    assert_eq!(results[0].terminal_revision, terminal.revision);
    assert_eq!(results[0].payload_digest, expected_digest);
    assert_eq!(results[0].state, "failed");
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "terminal-crash-goal".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "crash-delivery-worker".into(),
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

fn install_pid_recording_child_shim(env: &mut HubEnv) -> std::path::PathBuf {
    let shim = env.home.path().join("terminal-delivery-agent-shim");
    let pid = env.home.path().join("terminal-delivery-worker.pid");
    let script = r#"#!/bin/sh
state="$HOME/.loopal/terminal-delivery-root"
mkdir -p "$HOME/.loopal"
if mkdir "$state" 2>/dev/null; then
  exec "$LOOPAL_E2E_REAL_BINARY" "$@"
fi
printf '%s\n' "$$" > "$HOME/terminal-delivery-worker.pid"
exec "$LOOPAL_E2E_REAL_BINARY" "$@"
"#;
    std::fs::write(&shim, script).unwrap();
    let mut permissions = std::fs::metadata(&shim).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&shim, permissions).unwrap();
    env.agent_binary_override = Some(shim);
    pid
}

fn read_pid(path: &std::path::Path) -> String {
    std::fs::read_to_string(path)
        .expect("workflow worker PID")
        .trim()
        .to_string()
}

fn terminate(pid: String) {
    let status = std::process::Command::new("kill")
        .args(["-TERM", &pid])
        .status()
        .expect("signal workflow worker");
    assert!(
        status.success(),
        "failed to terminate workflow worker {pid}"
    );
}
