use super::reducer::{WorkflowReduceError, illegal};
use super::reducer_graph::{attempt_mut, finish, node, node_mut, require_current, skip_unstarted};
use super::*;

pub(crate) fn deadline(
    run: &mut WorkflowRunSnapshot,
    failure: &WorkflowAttemptFailure,
) -> Result<(), WorkflowReduceError> {
    if run.state != WorkflowRunState::Running {
        return Err(illegal("deadline requires running run"));
    }
    if run.nodes.iter().any(|node| {
        matches!(
            node.state,
            WorkflowNodeState::Dispatching
                | WorkflowNodeState::Running
                | WorkflowNodeState::Cancelling
        )
    }) {
        return Err(illegal("deadline requires no active attempts"));
    }
    run.failure.get_or_insert(failure.clone());
    skip_unstarted(run);
    finish(run)
}

pub(crate) fn stop(
    run: &mut WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
) -> Result<(), WorkflowReduceError> {
    if run.state != WorkflowRunState::Running {
        return Err(illegal("attempt stop requires running run"));
    }
    let node_state = node(run, node_id)?.state;
    if !matches!(
        node_state,
        WorkflowNodeState::Dispatching | WorkflowNodeState::Running
    ) {
        return Err(illegal("attempt stop requires active node"));
    }
    require_current(run, node_id, attempt_id, node_state)?;
    let attempt = attempt_mut(run, attempt_id)?;
    if !matches!(
        attempt.state,
        WorkflowAttemptState::Dispatching | WorkflowAttemptState::Running
    ) {
        return Err(illegal("attempt stop requires active attempt"));
    }
    attempt.state = WorkflowAttemptState::Cancelling;
    node_mut(run, node_id)?.state = WorkflowNodeState::Cancelling;
    Ok(())
}
