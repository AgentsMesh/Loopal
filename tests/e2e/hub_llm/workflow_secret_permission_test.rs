use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowExecution, WorkflowLimits, WorkflowOutput,
    WorkflowOutputContract, WorkflowPlanDecision, WorkflowRunState, WorkflowSpec,
    WorkflowWorkerProfileRef,
};
use serde_json::{Value, json};

use crate::support::{HubEnv, HubHarness, replay_workflow};
use crate::vault_test::seed_vault;

const PLAINTEXT: &str = "workflow-vault-plain-canary";
const TOOL_ID: &str = "workflow-secret-effect";

#[tokio::test]
async fn workflow_permission_receipt_cannot_bypass_closed_secret_acl() {
    let mut env = HubEnv::new();
    env.permission_mode = "ask_any_write".into();
    seed_vault(&env, "workflow_e2e_token", PLAINTEXT).await;
    let marker = env.cwd.path().join("workflow-secret-effect-marker");
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: spec() },
    })
    .unwrap();
    let mut harness = HubHarness::start_with_workflow_env(
        env,
        json!({
            "version": 3,
            "name": "workflow_secret_permission_boundary",
            "calls": [
                {"label": "planner", "expect": {"userContains": "workflow-secret-boundary-e2e"},
                 "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
                {"label": "worker-effect", "expect": {
                    "userContains": "workflow-secret-worker-canary",
                    "bodyExcludes": PLAINTEXT
                 }, "chunks": [
                    {"type": "tool_use", "id": TOOL_ID, "name": "Bash", "input": {
                        "command": "printf '%s' \"$TOKEN\" > \"$MARKER\"",
                        "env": {
                            "TOKEN": "<secret_ref:workflow_e2e_token>",
                            "MARKER": marker.to_string_lossy()
                        }
                    }},
                    {"type": "done"}
                ]},
                {"label": "worker-contained-denial", "expect": {
                    "toolResultId": TOOL_ID,
                    "bodyContains": "secret resolution failed",
                    "bodyExcludes": PLAINTEXT
                 }, "chunks": [
                    {"type": "text", "text": "workflow-secret-denial-contained"},
                    {"type": "done"}
                ]}
            ]
        }),
    )
    .await;

    let permission = harness.permission_client("permission-e2e").await;
    let approval = tokio::spawn(permission.approve_next("Bash".into()));
    let outcome = harness
        .workflow_turn(
            "Run workflow-secret-boundary-e2e with independent workers and cross-check it.",
        )
        .await;
    let (_permission, approval) = approval.await.expect("permission UI task");

    assert!(
        outcome.error.is_none(),
        "workflow input failed: {outcome:?}"
    );
    let terminal = outcome.summaries.last().expect("terminal summary");
    assert_eq!(terminal.state, WorkflowRunState::Succeeded);
    assert_eq!(approval.tool_name, "Bash");
    assert_eq!(
        approval.agent_name,
        format!("workflow-{}", approval.workflow.attempt_id)
    );
    assert_eq!(approval.workflow.run_id, terminal.id);
    assert_eq!(approval.workflow.node_id.as_str(), "worker");
    let approval_wire = approval.input.to_string();
    assert!(approval_wire.contains("<secret_ref:workflow_e2e_token>"));
    assert!(!approval_wire.contains(PLAINTEXT));
    assert!(!marker.exists(), "denied secret effect wrote {marker:?}");

    let replay = harness.workflow_replay(&terminal.id);
    let replay_wire = format!("{replay:?}");
    let run = replay_workflow(replay);
    assert_eq!(
        run.result,
        Some(WorkflowOutput::Text(
            "workflow-secret-denial-contained".into()
        ))
    );
    assert_eq!(run.attempts.len(), 1);
    assert_eq!(run.attempts[0].id, approval.workflow.attempt_id);

    let journal = harness.journal().await;
    assert_eq!(journal[2]["toolResultErrorIds"], json!([TOOL_ID]));
    assert!(
        journal
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["matched"] == true)
    );
    let audit = std::fs::read_to_string(harness.protected_audit_path()).unwrap();
    assert_workflow_audit(&audit, terminal.id.as_str(), run.attempts[0].id.as_str());
    for sink in [
        outcome.events.join("\n"),
        journal.to_string(),
        replay_wire,
        audit,
    ] {
        assert!(
            !sink.contains(PLAINTEXT),
            "plaintext leaked into sink: {sink}"
        );
    }
}

fn assert_workflow_audit(audit: &str, run_id: &str, attempt_id: &str) {
    let records = audit
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|record| record["tool_call_id"] == TOOL_ID)
        .collect::<Vec<_>>();
    let permission = records
        .iter()
        .find(|record| record["op"] == "permission_decision")
        .expect("workflow permission audit");
    let effect = records
        .iter()
        .find(|record| record["op"] == "tool_effect")
        .expect("workflow protected-effect audit");
    assert_eq!(permission["decision"], "allow");
    assert_eq!(permission["decision_source"], "ui");
    assert_eq!(effect["phase"], "pre_effect");
    for record in [permission, effect] {
        assert_eq!(record["workflow_run_id"], run_id);
        assert_eq!(record["workflow_node_id"], "worker");
        assert_eq!(record["workflow_attempt_id"], attempt_id);
        assert_eq!(record["tool_name"], "Bash");
        assert_eq!(record["depth"], 1);
        assert!(
            record["action_digest"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            record["schema_digest"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(record.get("action_input").is_none());
    }
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "workflow secret boundary".into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: "workflow-secret-worker-canary".into(),
            worker_profile: WorkflowWorkerProfileRef::new("default"),
        }],
        limits: WorkflowLimits {
            max_nodes: 1,
            max_parallel: 1,
            max_attempts: 1,
            run_deadline_ms: 30_000,
            attempt_timeout_ms: 15_000,
            max_output_bytes: 4_096,
        },
        output_node: "worker".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}
