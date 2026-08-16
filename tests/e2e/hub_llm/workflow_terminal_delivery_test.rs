use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowExecution, WorkflowLimits, WorkflowOutputContract,
    WorkflowPlanDecision, WorkflowRunState, WorkflowSpec, WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::HubHarness;

#[tokio::test]
async fn success_result_enters_root_conversation_and_is_acked_once() {
    let mut harness = HubHarness::start_with_workflow(scenario(
        "terminal-success-goal",
        text_spec("terminal-success-goal", "success-worker-canary"),
        "authoritative-success-result",
        "Workflow",
        "success-root-consumed",
    ))
    .await;

    let outcome = harness
        .workflow_turn("Run terminal-success-goal with independent workers and cross-check it.")
        .await;
    let terminal = outcome.summaries.last().expect("terminal summary").clone();
    assert_eq!(terminal.state, WorkflowRunState::Succeeded);
    let response = harness
        .wait_for_terminal_root_response("success-root-consumed")
        .await;
    assert!(response.contains("success-root-consumed"));

    let delivery = harness.wait_for_delivery_ack(&terminal.id).await;
    let results = harness.persisted_workflow_results(&terminal.id);
    assert_eq!(results.len(), 1, "workflow result turns: {results:?}");
    assert_eq!(results[0].session_id, harness.session_id);
    assert_eq!(results[0].run_id, terminal.id.as_str());
    assert_eq!(results[0].terminal_revision, terminal.revision);
    assert_eq!(results[0].state, "succeeded");
    assert!(results[0].content.contains("authoritative-success-result"));
    assert_eq!(delivery.terminal_revision, terminal.revision);
    assert_eq!(
        harness.workflow_replay(&terminal.id).delivery_acks,
        vec![delivery]
    );
    assert_labels(
        &harness.journal().await,
        &["planner", "worker", "terminal-root"],
    );
}

#[tokio::test]
async fn failure_result_enters_root_conversation_and_is_acked_once() {
    let mut harness = HubHarness::start_with_workflow(scenario(
        "terminal-failure-goal",
        json_spec("terminal-failure-goal", "failure-worker-canary"),
        "not-json",
        "not valid JSON",
        "failure-root-consumed",
    ))
    .await;

    let outcome = harness
        .workflow_turn("Run terminal-failure-goal with independent workers and cross-check it.")
        .await;
    let terminal = outcome.summaries.last().expect("terminal summary").clone();
    assert_eq!(terminal.state, WorkflowRunState::Failed);
    harness
        .wait_for_terminal_root_response("failure-root-consumed")
        .await;

    let delivery = harness.wait_for_delivery_ack(&terminal.id).await;
    let results = harness.persisted_workflow_results(&terminal.id);
    assert_eq!(results.len(), 1, "workflow result turns: {results:?}");
    assert_eq!(results[0].state, "failed");
    assert!(results[0].content.contains("not valid JSON"));
    assert_eq!(delivery.terminal_revision, terminal.revision);
    assert_eq!(
        harness.workflow_replay(&terminal.id).delivery_acks,
        vec![delivery]
    );
    assert_labels(
        &harness.journal().await,
        &["planner", "worker", "terminal-root"],
    );
}

fn scenario(
    goal: &str,
    spec: WorkflowSpec,
    output: &str,
    terminal: &str,
    reply: &str,
) -> serde_json::Value {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec },
    })
    .unwrap();
    json!({"version": 3, "name": goal, "calls": [
        {"label": "planner", "expect": {"userContains": goal},
         "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
        {"label": "worker", "expect": {"userContains": "worker-canary"},
         "chunks": [{"type": "text", "text": output}, {"type": "done"}]},
        {"label": "terminal-root", "expect": {"userContains": terminal},
         "chunks": [{"type": "text", "text": reply}, {"type": "done"}]}
    ]})
}

fn text_spec(goal: &str, task: &str) -> WorkflowSpec {
    spec(
        goal,
        task,
        WorkflowOutputContract::Text { max_bytes: 1_024 },
    )
}

fn json_spec(goal: &str, task: &str) -> WorkflowSpec {
    spec(
        goal,
        task,
        WorkflowOutputContract::Json {
            max_bytes: 1_024,
            schema: json!({"type": "object"}),
        },
    )
}

fn spec(goal: &str, task: &str, output_contract: WorkflowOutputContract) -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: goal.into(),
        nodes: vec![WorkflowAgentNode {
            id: "worker".into(),
            dependencies: Vec::new(),
            task: task.into(),
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
        output_contract,
    }
}

fn assert_labels(journal: &serde_json::Value, expected: &[&str]) {
    let labels = journal
        .as_array()
        .unwrap()
        .iter()
        .map(|call| call["callLabel"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(labels, expected, "journal: {journal}");
    assert!(
        journal
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["matched"] == true)
    );
}
