use super::reducer::{WorkflowReduceError, illegal};
use super::reducer_graph::{
    attempt, attempt_mut, finish, node, node_mut, release, require_current, skip_unstarted,
};
use super::retry::normalize_failure;
use super::*;

pub(crate) fn succeed<V: WorkflowJsonValidator>(
    run: &mut WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
    completion: &crate::AgentCompletion,
    output: &Option<WorkflowOutput>,
    validator: &V,
) -> Result<(), WorkflowReduceError> {
    require_current(run, node_id, attempt_id, WorkflowNodeState::Running)?;
    if !completion.is_success() {
        return Err(WorkflowReduceError::InvalidCompletion);
    }
    if node_id == &run.spec.output_node {
        validate_workflow_output(
            &run.spec.output_contract,
            output.as_ref().ok_or(WorkflowReduceError::MissingOutput)?,
            validator,
        )?;
    }
    let attempt = attempt_mut(run, attempt_id)?;
    if attempt.state != WorkflowAttemptState::Running {
        return Err(illegal("success requires running attempt"));
    }
    attempt.state = WorkflowAttemptState::Succeeded;
    attempt.completion = Some(completion.clone());
    attempt.output = output.clone();
    let node = node_mut(run, node_id)?;
    node.state = WorkflowNodeState::Succeeded;
    node.current_attempt = None;
    release(run);
    finish(run)
}

pub(crate) fn fail(
    run: &mut WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
    completion: &crate::AgentCompletion,
    failure: &WorkflowAttemptFailure,
) -> Result<(), WorkflowReduceError> {
    let node_state = node(run, node_id)?.state;
    if !matches!(
        node_state,
        WorkflowNodeState::Dispatching | WorkflowNodeState::Running | WorkflowNodeState::Cancelling
    ) {
        return Err(illegal("failure requires active node"));
    }
    require_current(run, node_id, attempt_id, node_state)?;
    if completion.is_success() {
        return Err(WorkflowReduceError::InvalidCompletion);
    }
    let entered = attempt(run, attempt_id)?.entered_running;
    let normalized = normalize_failure(entered, failure.clone());
    let attempt = attempt_mut(run, attempt_id)?;
    if !matches!(
        attempt.state,
        WorkflowAttemptState::Dispatching
            | WorkflowAttemptState::Running
            | WorkflowAttemptState::Cancelling
    ) {
        return Err(illegal("failure requires active attempt"));
    }
    attempt.state = WorkflowAttemptState::Failed;
    attempt.completion = Some(completion.clone());
    attempt.failure = Some(normalized.clone());
    let automatic = run.state == WorkflowRunState::Running
        && run.failure.is_none()
        && normalized.class == WorkflowFailureClass::TransientBeforeExecution
        && has_retry_capacity(run);
    let node = node_mut(run, node_id)?;
    node.current_attempt = None;
    node.state = if automatic {
        WorkflowNodeState::Ready
    } else {
        WorkflowNodeState::Failed
    };
    if !automatic {
        run.failure.get_or_insert(normalized);
        skip_unstarted(run);
        return finish(run);
    }
    Ok(())
}

fn has_retry_capacity(run: &WorkflowRunSnapshot) -> bool {
    let first_attempts = run
        .nodes
        .iter()
        .filter(|node| !node.state.is_terminal() && node.attempt_count == 0)
        .count();
    (run.spec.limits.max_attempts as usize).saturating_sub(run.attempts.len()) > first_attempts
}
