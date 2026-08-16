use super::reducer::{WorkflowReduceError, illegal};
use super::*;

pub(crate) fn node<'a>(
    run: &'a WorkflowRunSnapshot,
    id: &WorkflowNodeId,
) -> Result<&'a WorkflowNodeSnapshot, WorkflowReduceError> {
    run.nodes
        .iter()
        .find(|node| &node.id == id)
        .ok_or(WorkflowReduceError::UnknownNode)
}

pub(crate) fn node_mut<'a>(
    run: &'a mut WorkflowRunSnapshot,
    id: &WorkflowNodeId,
) -> Result<&'a mut WorkflowNodeSnapshot, WorkflowReduceError> {
    run.nodes
        .iter_mut()
        .find(|node| &node.id == id)
        .ok_or(WorkflowReduceError::UnknownNode)
}

pub(crate) fn attempt<'a>(
    run: &'a WorkflowRunSnapshot,
    id: &WorkflowAttemptId,
) -> Result<&'a WorkflowAttemptSnapshot, WorkflowReduceError> {
    run.attempts
        .iter()
        .find(|attempt| &attempt.id == id)
        .ok_or(WorkflowReduceError::UnknownAttempt)
}

pub(crate) fn attempt_mut<'a>(
    run: &'a mut WorkflowRunSnapshot,
    id: &WorkflowAttemptId,
) -> Result<&'a mut WorkflowAttemptSnapshot, WorkflowReduceError> {
    run.attempts
        .iter_mut()
        .find(|attempt| &attempt.id == id)
        .ok_or(WorkflowReduceError::UnknownAttempt)
}

pub(crate) fn require_current(
    run: &WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
    expected: WorkflowNodeState,
) -> Result<(), WorkflowReduceError> {
    let node = node(run, node_id)?;
    if node.state != expected {
        return Err(illegal("node state does not match event"));
    }
    if node.current_attempt.as_ref() != Some(attempt_id) {
        return Err(WorkflowReduceError::AttemptMismatch);
    }
    if attempt(run, attempt_id)?.node_id != *node_id {
        return Err(WorkflowReduceError::AttemptMismatch);
    }
    Ok(())
}

pub(crate) fn release(run: &mut WorkflowRunSnapshot) {
    loop {
        let states: Vec<_> = run
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.state))
            .collect();
        let mut changed = false;
        for node in &mut run.nodes {
            if node.state != WorkflowNodeState::Pending {
                continue;
            }
            let dependencies: Vec<_> = node
                .dependencies
                .iter()
                .filter_map(|id| states.iter().find(|(candidate, _)| candidate == id))
                .map(|(_, state)| *state)
                .collect();
            if dependencies
                .iter()
                .all(|state| *state == WorkflowNodeState::Succeeded)
            {
                node.state = WorkflowNodeState::Ready;
                changed = true;
            } else if dependencies.iter().any(|state| {
                matches!(
                    state,
                    WorkflowNodeState::Failed
                        | WorkflowNodeState::Cancelled
                        | WorkflowNodeState::Skipped
                )
            }) {
                node.state = WorkflowNodeState::Skipped;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

pub(crate) fn skip_unstarted(run: &mut WorkflowRunSnapshot) {
    for node in &mut run.nodes {
        if matches!(
            node.state,
            WorkflowNodeState::Pending | WorkflowNodeState::Ready
        ) {
            node.state = WorkflowNodeState::Skipped;
        }
    }
}

pub(crate) fn finish(run: &mut WorkflowRunSnapshot) -> Result<(), WorkflowReduceError> {
    if !run.nodes.iter().all(|node| node.state.is_terminal()) {
        return Ok(());
    }
    if run.state == WorkflowRunState::Cancelling
        && !run
            .nodes
            .iter()
            .any(|node| node.state == WorkflowNodeState::Failed)
    {
        run.state = WorkflowRunState::Cancelled;
        return Ok(());
    }
    if run
        .nodes
        .iter()
        .all(|node| node.state == WorkflowNodeState::Succeeded)
    {
        let result = run
            .attempts
            .iter()
            .rev()
            .find(|attempt| {
                attempt.node_id == run.spec.output_node
                    && attempt.state == WorkflowAttemptState::Succeeded
            })
            .and_then(|attempt| attempt.output.clone())
            .ok_or(WorkflowReduceError::MissingOutput)?;
        run.result = Some(result);
        run.state = WorkflowRunState::Succeeded;
    } else {
        run.state = WorkflowRunState::Failed;
    }
    Ok(())
}
