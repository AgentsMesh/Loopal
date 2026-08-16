use loopal_protocol::*;

use crate::workflow_support::*;

fn independent_spec(max_attempts: u32) -> WorkflowSpec {
    let mut spec = text_spec();
    spec.nodes.insert(1, node("independent", &[]));
    spec.limits.max_attempts = max_attempts;
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
fn retry_does_not_consume_pending_nodes_first_attempt() {
    let mut spec = text_spec();
    spec.limits.max_attempts = 2;
    let run = running(spec);
    let run = dispatch(&run, "source", "watt_source_1");
    let run = transient_failure(&run, "source", "watt_source_1");

    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Failed);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Skipped);
    assert_eq!(run.attempts.len(), 1);
    assert_eq!(run.nodes[1].attempt_count, 0);
}

#[test]
fn retry_is_allowed_when_all_first_attempts_still_fit() {
    let mut spec = text_spec();
    spec.limits.max_attempts = 3;
    let run = running(spec);
    let run = dispatch(&run, "source", "watt_source_1");
    let run = transient_failure(&run, "source", "watt_source_1");

    assert_eq!(run.state, WorkflowRunState::Running);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Ready);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Pending);
    assert!(run.failure.is_none());
}

#[test]
fn retry_reserves_ready_and_pending_nodes_first_attempts() {
    let run = running(independent_spec(3));
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Ready);
    let run = dispatch(&run, "source", "watt_source_1");
    let run = transient_failure(&run, "source", "watt_source_1");

    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Failed);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Skipped);
    assert_eq!(run.nodes[2].state, WorkflowNodeState::Skipped);
    assert_eq!(run.nodes[1].attempt_count, 0);
    assert_eq!(run.nodes[2].attempt_count, 0);
}

#[test]
fn retry_does_not_reserve_for_active_sibling_with_an_attempt() {
    let run = running(independent_spec(4));
    let run = dispatch(&run, "source", "watt_source_1");
    let run = dispatch(&run, "independent", "watt_independent_1");
    let run = transient_failure(&run, "source", "watt_source_1");

    assert_eq!(run.state, WorkflowRunState::Running);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Ready);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Dispatching);
    assert_eq!(run.nodes[1].attempt_count, 1);
    assert_eq!(run.nodes[2].state, WorkflowNodeState::Pending);
    assert!(run.failure.is_none());
}
