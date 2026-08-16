use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{
    AgentCompletion, QualifiedAddress, WORKFLOW_SPEC_V1, WorkflowAgentNode,
    WorkflowAttemptCapability, WorkflowAttemptId, WorkflowAttemptSnapshot, WorkflowAttemptState,
    WorkflowFailureClass, WorkflowLimits, WorkflowNodeId, WorkflowNodeState,
    WorkflowOutputContract, WorkflowRunId, WorkflowRunSnapshot, WorkflowSpec,
    WorkflowWorkerProfileRef,
};

use super::*;

#[test]
fn dependency_results_preserve_declared_order_and_are_redacted() {
    let mut run = run();
    succeed(&mut run, "left", Some("left-result"));
    succeed(&mut run, "right", Some("right contains workflow-secret"));
    let seed = FinalSinkRedactionSeed::new();
    seed.observe("token", "workflow-secret".into()).unwrap();

    let results = resolve_dependency_results(&run, &WorkflowNodeId::new("join"), &seed).unwrap();

    assert_eq!(
        results,
        vec![
            WorkflowDependencyResult {
                node_id: WorkflowNodeId::new("right"),
                result: "right contains <secret_ref:token>".into(),
            },
            WorkflowDependencyResult {
                node_id: WorkflowNodeId::new("left"),
                result: "left-result".into(),
            },
        ]
    );
}

#[test]
fn missing_or_oversized_authoritative_result_fails_closed() {
    for result in [None, Some("x".repeat(65))] {
        let mut run = run();
        succeed(&mut run, "left", Some("left-result"));
        succeed(&mut run, "right", result.as_deref());

        let failure = resolve_dependency_results(
            &run,
            &WorkflowNodeId::new("join"),
            &FinalSinkRedactionSeed::new(),
        )
        .unwrap_err();

        assert_eq!(
            failure.completion.reason,
            "workflow_dependency_result_unavailable"
        );
        assert_eq!(failure.failure.class, WorkflowFailureClass::Permanent);
        assert!(failure.failure.reason.contains("right"));
    }
}

#[test]
fn dependency_result_can_use_the_workflow_limit_above_the_generic_agent_limit() {
    let mut run = run();
    let limit = loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES + 1;
    run.spec.limits.max_output_bytes = limit as u32;
    succeed(&mut run, "left", Some("left-result"));
    let expected = "w".repeat(limit);
    succeed(&mut run, "right", Some(&expected));

    let results = resolve_dependency_results(
        &run,
        &WorkflowNodeId::new("join"),
        &FinalSinkRedactionSeed::new(),
    )
    .unwrap();

    assert_eq!(results[0].result, expected);
}

#[test]
fn actual_dependency_result_aggregate_fails_closed() {
    let dependency_ids: Vec<String> = (0..9).map(|index| format!("source_{index}")).collect();
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_dependency_budget"),
        QualifiedAddress::local("root"),
        WorkflowSpec {
            version: WORKFLOW_SPEC_V1,
            run_goal: "join results".into(),
            nodes: dependency_ids
                .iter()
                .map(|id| node(id, &[]))
                .chain(std::iter::once(node(
                    "join",
                    &dependency_ids
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                )))
                .collect(),
            limits: WorkflowLimits {
                max_nodes: 10,
                max_parallel: 9,
                max_attempts: 10,
                run_deadline_ms: 60_000,
                attempt_timeout_ms: 30_000,
                max_output_bytes: loopal_protocol::MAX_WORKFLOW_OUTPUT_BYTES,
            },
            output_node: WorkflowNodeId::new("join"),
            output_contract: WorkflowOutputContract::Text {
                max_bytes: loopal_protocol::MAX_WORKFLOW_OUTPUT_BYTES,
            },
        },
        1,
    );
    run.nodes.last_mut().unwrap().state = WorkflowNodeState::Ready;
    let result = "x".repeat(loopal_protocol::MAX_WORKFLOW_OUTPUT_BYTES as usize);
    for dependency_id in &dependency_ids {
        succeed(&mut run, dependency_id, Some(&result));
    }

    let failure = resolve_dependency_results(
        &run,
        &WorkflowNodeId::new("join"),
        &FinalSinkRedactionSeed::new(),
    )
    .unwrap_err();

    assert_eq!(
        failure.completion.reason,
        "workflow_dependency_results_too_large"
    );
    assert_eq!(failure.failure.class, WorkflowFailureClass::Permanent);
    assert!(failure.failure.reason.contains("join"));
}

fn succeed(run: &mut WorkflowRunSnapshot, node_id: &str, result: Option<&str>) {
    run.nodes
        .iter_mut()
        .find(|node| node.id.as_str() == node_id)
        .unwrap()
        .state = WorkflowNodeState::Succeeded;
    run.attempts.push(WorkflowAttemptSnapshot {
        id: WorkflowAttemptId::new(format!("watt_{node_id}")),
        node_id: WorkflowNodeId::new(node_id),
        capability_digest: WorkflowAttemptCapability::parse("11".repeat(32))
            .unwrap()
            .digest(),
        dispatched_at_unix_ms: 1,
        state: WorkflowAttemptState::Succeeded,
        agent: None,
        entered_running: true,
        completion: Some(AgentCompletion::goal(result.map(str::to_owned))),
        failure: None,
        output: None,
    });
}

fn run() -> WorkflowRunSnapshot {
    let mut run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_dependencies"),
        QualifiedAddress::local("root"),
        WorkflowSpec {
            version: WORKFLOW_SPEC_V1,
            run_goal: "join results".into(),
            nodes: vec![
                node("left", &[]),
                node("right", &[]),
                node("join", &["right", "left"]),
            ],
            limits: WorkflowLimits {
                max_nodes: 3,
                max_parallel: 2,
                max_attempts: 3,
                run_deadline_ms: 60_000,
                attempt_timeout_ms: 30_000,
                max_output_bytes: 64,
            },
            output_node: WorkflowNodeId::new("join"),
            output_contract: WorkflowOutputContract::Text { max_bytes: 64 },
        },
        1,
    );
    run.nodes[2].state = WorkflowNodeState::Ready;
    run
}

fn node(id: &str, dependencies: &[&str]) -> WorkflowAgentNode {
    WorkflowAgentNode {
        id: WorkflowNodeId::new(id),
        dependencies: dependencies.iter().copied().map(Into::into).collect(),
        task: format!("complete {id}"),
        worker_profile: WorkflowWorkerProfileRef::new("default"),
    }
}
