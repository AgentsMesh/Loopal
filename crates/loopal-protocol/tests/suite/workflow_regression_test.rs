use loopal_protocol::*;

use crate::workflow_support::*;

fn independent_spec() -> WorkflowSpec {
    let mut spec = text_spec();
    spec.nodes.insert(1, node("independent", &[]));
    spec.limits.max_attempts = 3;
    spec
}

fn transient_failure(
    run: &WorkflowRunSnapshot,
    node_id: &str,
    attempt_id: &str,
) -> WorkflowRunSnapshot {
    apply(
        run,
        WorkflowEventPayload::AttemptFailed {
            node_id: node_id.into(),
            attempt_id: attempt_id.into(),
            completion: AgentCompletion::new("transport_error", Some("failed".into())),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::TransientBeforeExecution,
                reason: "failed before execution".into(),
            },
        },
    )
}

#[test]
fn permanent_failure_skips_pending_and_independent_ready_nodes() {
    let run = running(independent_spec());
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Ready);
    let run = dispatch(&run, "source", "watt_source");
    let run = apply(
        &run,
        WorkflowEventPayload::AttemptFailed {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            completion: AgentCompletion::new("error", None),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "permanent".into(),
            },
        },
    );
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Failed);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Skipped);
    assert_eq!(run.nodes[2].state, WorkflowNodeState::Skipped);
}

#[test]
fn sibling_transient_failure_does_not_retry_after_run_is_doomed() {
    let mut spec = independent_spec();
    spec.nodes.pop();
    spec.output_node = "independent".into();
    let run = running(spec);
    let run = dispatch(&run, "source", "watt_source");
    let run = dispatch(&run, "independent", "watt_independent");
    let run = apply(
        &run,
        WorkflowEventPayload::AttemptFailed {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            completion: AgentCompletion::new("error", None),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "permanent".into(),
            },
        },
    );
    assert_eq!(run.state, WorkflowRunState::Running);
    let run = transient_failure(&run, "independent", "watt_independent");
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert!(
        run.nodes
            .iter()
            .all(|node| node.state != WorkflowNodeState::Ready)
    );
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Failed);
}

#[test]
fn successful_output_retry_supplies_terminal_run_result() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = succeed(&run, "source", "watt_source", None);

    let run = dispatch(&run, "output", "watt_output_1");
    let run = transient_failure(&run, "output", "watt_output_1");
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Ready);

    let run = dispatch(&run, "output", "watt_output_2");
    let run = bind_and_run(&run, "output", "watt_output_2");
    let run = succeed(
        &run,
        "output",
        "watt_output_2",
        Some(WorkflowOutput::Text("retry result".into())),
    );
    assert_eq!(run.state, WorkflowRunState::Succeeded);
    assert_eq!(
        run.result,
        Some(WorkflowOutput::Text("retry result".into()))
    );
}
