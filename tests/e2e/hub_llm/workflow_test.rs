use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowAgentNode, WorkflowExecution, WorkflowLimits, WorkflowOutput,
    WorkflowOutputContract, WorkflowPlanDecision, WorkflowRunState, WorkflowSpec,
    WorkflowWorkerProfileRef,
};
use serde_json::json;

use crate::support::{HubHarness, replay_workflow};

#[tokio::test]
async fn proactive_root_runs_parallel_workers_join_and_commits_terminal_output() {
    let plan = serde_json::to_string(&WorkflowPlanDecision {
        version: loopal_protocol::WORKFLOW_PLAN_V1,
        execution: WorkflowExecution::Workflow { spec: spec() },
    })
    .unwrap();
    let join_contract = concat!(
        "Authoritative dependency results (JSON, in declared dependency order):\n",
        "[{\"node_id\":\"left\",\"result\":\"left-result\"},{\"node_id\":\"right\",",
        "\"result\":\"right-result\"}]\n\n",
        "Output contract (authoritative):\n",
        "Return exactly one final plain-text value in the agent completion result field. ",
        "The UTF-8 result must be no longer than 1024 bytes. ",
        "Return only that text, without JSON encoding or Markdown fences."
    );
    let mut harness = HubHarness::start_with_workflow(json!({
        "version": 3,
        "name": "production_workflow_happy_path",
        "calls": [
            {"label": "planner", "expect": {"userContains": "parallel-e2e-goal"},
             "chunks": [{"type": "text", "text": plan}, {"type": "done"}]},
            {"label": "left-worker", "expect": {"userContains": "left-worker-canary"},
             "delayMs": 200,
             "chunks": [{"type": "text", "text": "left-result"}, {"type": "done"}]},
            {"label": "right-worker", "expect": {"userContains": "right-worker-canary"},
             "delayMs": 200,
             "chunks": [{"type": "text", "text": "right-result"}, {"type": "done"}]},
            {"label": "join-worker", "expect": {
                 "userContains": join_contract,
                 "bodyContains": "join-worker-canary"
             },
             "chunks": [{"type": "text", "text": "joined-terminal-output"}, {"type": "done"}]}
        ]
    }))
    .await;

    let outcome = harness
        .workflow_turn(
            "Run parallel-e2e-goal with multiple agents independently, then join their work.",
        )
        .await;
    assert!(
        outcome.error.is_none(),
        "workflow input failed: {outcome:?}"
    );
    let terminal = outcome
        .summaries
        .last()
        .expect("workflow must publish a terminal summary");
    assert_eq!(terminal.state, WorkflowRunState::Succeeded);
    assert_eq!(terminal.counts.succeeded, 3);
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.counts.active == 2),
        "the two root nodes never ran in parallel: {:?}",
        outcome.summaries
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.counts.succeeded == 2 && summary.counts.active == 1),
        "the join did not start after both roots committed: {:?}",
        outcome.summaries
    );

    let run = replay_workflow(harness.workflow_replay(&terminal.id));
    assert_eq!(run.state, WorkflowRunState::Succeeded);
    assert_eq!(
        run.result,
        Some(WorkflowOutput::Text("joined-terminal-output".into()))
    );
    assert_eq!(run.attempts.len(), 3);
    assert!(run.attempts.iter().all(|attempt| attempt.entered_running));

    let requests = harness.journal().await;
    assert_eq!(requests.as_array().map(Vec::len), Some(4), "{requests}");
    assert_eq!(requests[0]["callLabel"], "planner");
    assert_eq!(requests[3]["callLabel"], "join-worker");
    assert!(
        requests
            .as_array()
            .unwrap()
            .iter()
            .all(|call| call["matched"] == true)
    );
}

fn spec() -> WorkflowSpec {
    WorkflowSpec {
        version: WORKFLOW_SPEC_V1,
        run_goal: "production parallel workflow".into(),
        nodes: vec![
            node("left", &[], "left-worker-canary"),
            node("right", &[], "right-worker-canary"),
            node("join", &["left", "right"], "join-worker-canary"),
        ],
        limits: WorkflowLimits {
            max_nodes: 3,
            max_parallel: 2,
            max_attempts: 3,
            run_deadline_ms: 60_000,
            attempt_timeout_ms: 30_000,
            max_output_bytes: 4_096,
        },
        output_node: "join".into(),
        output_contract: WorkflowOutputContract::Text { max_bytes: 1_024 },
    }
}

fn node(id: &str, dependencies: &[&str], task: &str) -> WorkflowAgentNode {
    WorkflowAgentNode {
        id: id.into(),
        dependencies: dependencies.iter().copied().map(Into::into).collect(),
        task: task.into(),
        worker_profile: WorkflowWorkerProfileRef::new("default"),
    }
}
