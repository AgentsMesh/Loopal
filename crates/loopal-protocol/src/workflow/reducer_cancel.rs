use super::reducer::{WorkflowReduceError, illegal};
use super::reducer_graph::{attempt_mut, finish, node_mut, require_current};
use super::*;

pub(crate) fn cancel(run: &mut WorkflowRunSnapshot) -> Result<(), WorkflowReduceError> {
    if !matches!(
        run.state,
        WorkflowRunState::Planned | WorkflowRunState::Validated | WorkflowRunState::Running
    ) {
        return Err(illegal("cancel requires non-terminal admissible run"));
    }
    run.state = WorkflowRunState::Cancelling;
    for node in &mut run.nodes {
        match node.state {
            WorkflowNodeState::Pending | WorkflowNodeState::Ready => {
                node.state = WorkflowNodeState::Cancelled;
            }
            WorkflowNodeState::Dispatching | WorkflowNodeState::Running => {
                node.state = WorkflowNodeState::Cancelling;
            }
            _ => {}
        }
    }
    for attempt in &mut run.attempts {
        if matches!(
            attempt.state,
            WorkflowAttemptState::Dispatching | WorkflowAttemptState::Running
        ) {
            attempt.state = WorkflowAttemptState::Cancelling;
        }
    }
    finish(run)
}

pub(crate) fn cancelled(
    run: &mut WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
) -> Result<(), WorkflowReduceError> {
    if run.state != WorkflowRunState::Cancelling {
        return Err(illegal("attempt cancel requires cancelling run"));
    }
    require_current(run, node_id, attempt_id, WorkflowNodeState::Cancelling)?;
    let attempt = attempt_mut(run, attempt_id)?;
    if attempt.state != WorkflowAttemptState::Cancelling {
        return Err(illegal("attempt is not cancelling"));
    }
    attempt.state = WorkflowAttemptState::Cancelled;
    let node = node_mut(run, node_id)?;
    node.state = WorkflowNodeState::Cancelled;
    node.current_attempt = None;
    finish(run)
}
