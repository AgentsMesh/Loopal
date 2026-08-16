use loopal_output_guard::{FinalSinkRedactionSeed, OutputGuard};
use loopal_protocol::{
    AgentCompletion, MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES, WorkflowAttemptFailure,
    WorkflowAttemptState, WorkflowFailureClass, WorkflowNodeId, WorkflowNodeState,
    WorkflowRunSnapshot,
};

use super::WorkflowSpawnFailure;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkflowDependencyResult {
    pub(crate) node_id: WorkflowNodeId,
    pub(crate) result: String,
}

pub(in crate::workflow) fn resolve_dependency_results(
    run: &WorkflowRunSnapshot,
    node_id: &WorkflowNodeId,
    seed: &FinalSinkRedactionSeed,
) -> Result<Vec<WorkflowDependencyResult>, WorkflowSpawnFailure> {
    let node = run
        .nodes
        .iter()
        .find(|node| &node.id == node_id)
        .ok_or_else(|| unavailable(node_id))?;
    let snapshot = seed.snapshot().map_err(|_| unavailable(node_id))?;
    let guard = OutputGuard::new(&snapshot).map_err(|_| unavailable(node_id))?;
    let limit = run.spec.limits.max_output_bytes as usize;

    node.dependencies
        .iter()
        .try_fold(
            (Vec::with_capacity(node.dependencies.len()), 0usize),
            |(mut results, total_bytes), dependency_id| {
                let dependency = run
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id == *dependency_id)
                    .filter(|dependency| dependency.state == WorkflowNodeState::Succeeded)
                    .ok_or_else(|| unavailable(dependency_id))?;
                let result = run
                    .attempts
                    .iter()
                    .rev()
                    .find(|attempt| {
                        attempt.node_id == dependency.id
                            && attempt.state == WorkflowAttemptState::Succeeded
                    })
                    .and_then(|attempt| attempt.completion.as_ref())
                    .filter(|completion| completion.is_success())
                    .and_then(|completion| completion.result.as_deref())
                    .ok_or_else(|| unavailable(dependency_id))?;
                let result = guard
                    .guard_text(result, limit)
                    .map_err(|_| unavailable(dependency_id))?
                    .into_inner()
                    .into_string();
                let total_bytes = total_bytes
                    .checked_add(result.len())
                    .filter(|total| *total <= MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES as usize)
                    .ok_or_else(|| aggregate_too_large(node_id))?;
                results.push(WorkflowDependencyResult {
                    node_id: dependency_id.clone(),
                    result,
                });
                Ok((results, total_bytes))
            },
        )
        .map(|(results, _)| results)
}

fn aggregate_too_large(node_id: &WorkflowNodeId) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_dependency_results_too_large", None),
        failure: WorkflowAttemptFailure {
            class: WorkflowFailureClass::Permanent,
            reason: format!(
                "workflow dependencies for {node_id} exceed the aggregate result byte limit"
            ),
        },
    }
}

fn unavailable(node_id: &WorkflowNodeId) -> WorkflowSpawnFailure {
    WorkflowSpawnFailure {
        completion: AgentCompletion::new("workflow_dependency_result_unavailable", None),
        failure: WorkflowAttemptFailure {
            class: WorkflowFailureClass::Permanent,
            reason: format!(
                "workflow dependency {node_id} has no usable authoritative completion result"
            ),
        },
    }
}

#[cfg(test)]
#[path = "dependencies_tests.rs"]
mod tests;
