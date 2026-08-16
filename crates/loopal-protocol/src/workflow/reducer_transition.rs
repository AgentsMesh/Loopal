use super::reducer::{WorkflowReduceError, illegal};
use super::reducer_attempt::{fail, succeed};
use super::reducer_cancel::{cancel, cancelled};
use super::reducer_graph::*;
use super::reducer_stop::{deadline, stop};
use super::*;
use crate::WorkflowAttemptCapabilityDigest;

pub(crate) fn apply<V: WorkflowJsonValidator>(
    run: &mut WorkflowRunSnapshot,
    event: &WorkflowEventPayload,
    validator: &V,
) -> Result<(), WorkflowReduceError> {
    match event {
        WorkflowEventPayload::SpecValidated => validate(run),
        WorkflowEventPayload::RunStarted => start(run),
        WorkflowEventPayload::RunDeadlineExceeded { failure } => deadline(run, failure),
        WorkflowEventPayload::DispatchIntended {
            node_id,
            attempt_id,
            capability_digest,
        } => dispatch(run, node_id, attempt_id, *capability_digest),
        WorkflowEventPayload::AttemptBound {
            node_id,
            attempt_id,
            agent,
        } => bind(run, node_id, attempt_id, agent),
        WorkflowEventPayload::AttemptRunning {
            node_id,
            attempt_id,
        } => running(run, node_id, attempt_id),
        WorkflowEventPayload::AttemptStopRequested {
            node_id,
            attempt_id,
            ..
        } => stop(run, node_id, attempt_id),
        WorkflowEventPayload::AttemptSucceeded {
            node_id,
            attempt_id,
            completion,
            output,
        } => succeed(run, node_id, attempt_id, completion, output, validator),
        WorkflowEventPayload::AttemptFailed {
            node_id,
            attempt_id,
            completion,
            failure,
        } => fail(run, node_id, attempt_id, completion, failure),
        WorkflowEventPayload::CancelRequested { .. } => cancel(run),
        WorkflowEventPayload::AttemptCancelled {
            node_id,
            attempt_id,
            ..
        } => cancelled(run, node_id, attempt_id),
    }
}

fn validate(run: &mut WorkflowRunSnapshot) -> Result<(), WorkflowReduceError> {
    if run.state != WorkflowRunState::Planned {
        return Err(illegal("validation requires planned run"));
    }
    validate_workflow_spec(&run.spec)?;
    run.state = WorkflowRunState::Validated;
    Ok(())
}

fn start(run: &mut WorkflowRunSnapshot) -> Result<(), WorkflowReduceError> {
    if run.state != WorkflowRunState::Validated {
        return Err(illegal("start requires validated run"));
    }
    run.state = WorkflowRunState::Running;
    for node in &mut run.nodes {
        if node.dependencies.is_empty() {
            node.state = WorkflowNodeState::Ready;
        }
    }
    Ok(())
}

fn dispatch(
    run: &mut WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
    capability_digest: WorkflowAttemptCapabilityDigest,
) -> Result<(), WorkflowReduceError> {
    if !attempt_id.is_valid() {
        return Err(WorkflowReduceError::InvalidAttemptId);
    }
    if run.state != WorkflowRunState::Running {
        return Err(illegal("dispatch requires running run"));
    }
    if run.attempts.iter().any(|attempt| &attempt.id == attempt_id) {
        return Err(WorkflowReduceError::AttemptExists);
    }
    if run.attempts.len() >= run.spec.limits.max_attempts as usize {
        return Err(WorkflowReduceError::AttemptsExhausted);
    }
    let active = run
        .attempts
        .iter()
        .filter(|attempt| !attempt.state.is_terminal())
        .count();
    if active >= run.spec.limits.max_parallel as usize {
        return Err(illegal("max_parallel admission exhausted"));
    }
    let node = node_mut(run, node_id)?;
    if node.state != WorkflowNodeState::Ready {
        return Err(illegal("dispatch requires ready node"));
    }
    node.state = WorkflowNodeState::Dispatching;
    node.current_attempt = Some(attempt_id.clone());
    node.attempt_count += 1;
    run.attempts.push(WorkflowAttemptSnapshot {
        id: attempt_id.clone(),
        node_id: node_id.clone(),
        capability_digest,
        dispatched_at_unix_ms: 0,
        state: WorkflowAttemptState::Dispatching,
        agent: None,
        entered_running: false,
        completion: None,
        failure: None,
        output: None,
    });
    Ok(())
}

fn bind(
    run: &mut WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
    agent: &crate::QualifiedAddress,
) -> Result<(), WorkflowReduceError> {
    let node_state = node(run, node_id)?.state;
    if !matches!(
        node_state,
        WorkflowNodeState::Dispatching | WorkflowNodeState::Cancelling
    ) {
        return Err(illegal("binding requires active node"));
    }
    require_current(run, node_id, attempt_id, node_state)?;
    let attempt = attempt_mut(run, attempt_id)?;
    if !matches!(
        attempt.state,
        WorkflowAttemptState::Dispatching | WorkflowAttemptState::Cancelling
    ) || attempt.agent.is_some()
    {
        return Err(illegal("binding requires unbound active attempt"));
    }
    attempt.agent = Some(agent.clone());
    Ok(())
}

fn running(
    run: &mut WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    attempt_id: &WorkflowAttemptId,
) -> Result<(), WorkflowReduceError> {
    require_current(run, node_id, attempt_id, WorkflowNodeState::Dispatching)?;
    let attempt = attempt_mut(run, attempt_id)?;
    if attempt.state != WorkflowAttemptState::Dispatching || attempt.agent.is_none() {
        return Err(illegal("running requires bound dispatching attempt"));
    }
    attempt.state = WorkflowAttemptState::Running;
    attempt.entered_running = true;
    node_mut(run, node_id)?.state = WorkflowNodeState::Running;
    Ok(())
}
