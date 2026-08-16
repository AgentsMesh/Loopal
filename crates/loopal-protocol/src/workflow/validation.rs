use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::node_validation::validate_nodes;
use super::schema_validation::validate_workflow_schema_bounds;
use super::spec::*;
use super::{WorkflowNodeId, WorkflowSpec};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowValidationError {
    UnsupportedVersion {
        version: u16,
    },
    EmptyGoal,
    GoalTooLong,
    EmptyGraph,
    SpecTooLarge,
    NodeLimit {
        actual: usize,
        configured: u32,
    },
    InvalidLimit {
        field: String,
    },
    InvalidNodeId {
        node_id: WorkflowNodeId,
    },
    DuplicateNodeId {
        node_id: WorkflowNodeId,
    },
    EmptyTask {
        node_id: WorkflowNodeId,
    },
    TaskTooLong {
        node_id: WorkflowNodeId,
    },
    InvalidWorkerProfile {
        node_id: WorkflowNodeId,
    },
    TooManyDependencies {
        node_id: WorkflowNodeId,
    },
    DependencyOutputBudgetExceeded {
        node_id: WorkflowNodeId,
        potential_bytes: u64,
        max_bytes: u64,
    },
    DuplicateDependency {
        node_id: WorkflowNodeId,
        dependency: WorkflowNodeId,
    },
    SelfDependency {
        node_id: WorkflowNodeId,
    },
    MissingDependency {
        node_id: WorkflowNodeId,
        dependency: WorkflowNodeId,
    },
    Cycle,
    MissingOutputNode {
        node_id: WorkflowNodeId,
    },
    InvalidOutputBound,
    SchemaRootNotObject,
    SchemaTooLarge,
    SchemaTooDeep,
    SchemaTooComplex,
    SchemaKeyTooLong,
    SchemaStringTooLong,
}

pub fn validate_workflow_spec(spec: &WorkflowSpec) -> Result<(), WorkflowValidationError> {
    if serde_json::to_vec(spec)
        .expect("WorkflowSpec always serializes")
        .len()
        > MAX_WORKFLOW_SPEC_BYTES
    {
        return Err(WorkflowValidationError::SpecTooLarge);
    }
    validate_header(spec)?;
    let nodes = validate_nodes(spec)?;
    validate_acyclic(spec, &nodes)?;
    validate_output(spec)?;
    Ok(())
}

fn validate_header(spec: &WorkflowSpec) -> Result<(), WorkflowValidationError> {
    if spec.version != WORKFLOW_SPEC_V1 {
        return Err(WorkflowValidationError::UnsupportedVersion {
            version: spec.version,
        });
    }
    if spec.run_goal.trim().is_empty() {
        return Err(WorkflowValidationError::EmptyGoal);
    }
    if spec.run_goal.len() > MAX_WORKFLOW_GOAL_BYTES {
        return Err(WorkflowValidationError::GoalTooLong);
    }
    if spec.nodes.is_empty() {
        return Err(WorkflowValidationError::EmptyGraph);
    }
    validate_limits(spec)
}

fn validate_limits(spec: &WorkflowSpec) -> Result<(), WorkflowValidationError> {
    let limits = &spec.limits;
    let checks = [
        (
            limits.max_nodes > 0 && limits.max_nodes <= MAX_WORKFLOW_NODES,
            "max_nodes",
        ),
        (
            limits.max_parallel > 0
                && limits.max_parallel <= MAX_WORKFLOW_PARALLELISM
                && limits.max_parallel <= limits.max_nodes,
            "max_parallel",
        ),
        (
            limits.max_attempts >= spec.nodes.len() as u32
                && limits.max_attempts <= MAX_WORKFLOW_ATTEMPTS,
            "max_attempts",
        ),
        (
            limits.run_deadline_ms > 0 && limits.run_deadline_ms <= MAX_WORKFLOW_RUN_DEADLINE_MS,
            "run_deadline_ms",
        ),
        (
            limits.attempt_timeout_ms > 0
                && limits.attempt_timeout_ms <= MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS
                && limits.attempt_timeout_ms <= limits.run_deadline_ms,
            "attempt_timeout_ms",
        ),
        (
            limits.max_output_bytes > 0 && limits.max_output_bytes <= MAX_WORKFLOW_OUTPUT_BYTES,
            "max_output_bytes",
        ),
    ];
    for (valid, field) in checks {
        if !valid {
            return Err(WorkflowValidationError::InvalidLimit {
                field: field.into(),
            });
        }
    }
    if spec.nodes.len() > limits.max_nodes as usize {
        return Err(WorkflowValidationError::NodeLimit {
            actual: spec.nodes.len(),
            configured: limits.max_nodes,
        });
    }
    Ok(())
}

fn validate_acyclic(
    spec: &WorkflowSpec,
    nodes: &BTreeMap<&WorkflowNodeId, &WorkflowAgentNode>,
) -> Result<(), WorkflowValidationError> {
    let mut indegree: BTreeMap<&WorkflowNodeId, usize> = nodes.keys().map(|id| (*id, 0)).collect();
    for node in &spec.nodes {
        *indegree.get_mut(&node.id).expect("validated node") = node.dependencies.len();
    }
    let mut ready: Vec<&WorkflowNodeId> = indegree
        .iter()
        .filter_map(|(id, n)| (*n == 0).then_some(*id))
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop() {
        visited += 1;
        for node in &spec.nodes {
            if node.dependencies.contains(id) {
                let count = indegree.get_mut(&node.id).expect("validated node");
                *count -= 1;
                if *count == 0 {
                    ready.push(&node.id);
                }
            }
        }
    }
    (visited == spec.nodes.len())
        .then_some(())
        .ok_or(WorkflowValidationError::Cycle)
}

fn validate_output(spec: &WorkflowSpec) -> Result<(), WorkflowValidationError> {
    if !spec.nodes.iter().any(|node| node.id == spec.output_node) {
        return Err(WorkflowValidationError::MissingOutputNode {
            node_id: spec.output_node.clone(),
        });
    }
    let max = spec.output_contract.max_bytes();
    if max == 0 || max > spec.limits.max_output_bytes {
        return Err(WorkflowValidationError::InvalidOutputBound);
    }
    if let WorkflowOutputContract::Json { schema, .. } = &spec.output_contract {
        validate_workflow_schema_bounds(schema)?;
    }
    Ok(())
}
