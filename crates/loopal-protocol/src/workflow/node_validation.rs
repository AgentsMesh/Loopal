use std::collections::{BTreeMap, BTreeSet};

use super::ids::valid_workflow_id;
use super::spec::*;
use super::{WorkflowNodeId, WorkflowSpec, WorkflowValidationError};

pub(crate) fn validate_nodes(
    spec: &WorkflowSpec,
) -> Result<BTreeMap<&WorkflowNodeId, &WorkflowAgentNode>, WorkflowValidationError> {
    let mut nodes = BTreeMap::new();
    for node in &spec.nodes {
        if !valid_workflow_id(node.id.as_str()) {
            return Err(WorkflowValidationError::InvalidNodeId {
                node_id: node.id.clone(),
            });
        }
        if nodes.insert(&node.id, node).is_some() {
            return Err(WorkflowValidationError::DuplicateNodeId {
                node_id: node.id.clone(),
            });
        }
        validate_node_content(node, spec.limits.max_output_bytes)?;
    }
    validate_dependencies(spec, &nodes)?;
    Ok(nodes)
}

fn validate_node_content(
    node: &WorkflowAgentNode,
    max_output_bytes: u32,
) -> Result<(), WorkflowValidationError> {
    if node.task.trim().is_empty() {
        return Err(WorkflowValidationError::EmptyTask {
            node_id: node.id.clone(),
        });
    }
    if node.task.len() > MAX_WORKFLOW_TASK_BYTES {
        return Err(WorkflowValidationError::TaskTooLong {
            node_id: node.id.clone(),
        });
    }
    if !node.worker_profile.is_valid() {
        return Err(WorkflowValidationError::InvalidWorkerProfile {
            node_id: node.id.clone(),
        });
    }
    if node.dependencies.len() > MAX_DEPENDENCIES_PER_NODE {
        return Err(WorkflowValidationError::TooManyDependencies {
            node_id: node.id.clone(),
        });
    }
    let potential_bytes = u64::try_from(node.dependencies.len())
        .ok()
        .and_then(|count| count.checked_mul(u64::from(max_output_bytes)))
        .ok_or_else(|| WorkflowValidationError::DependencyOutputBudgetExceeded {
            node_id: node.id.clone(),
            potential_bytes: u64::MAX,
            max_bytes: MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES,
        })?;
    if potential_bytes > MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES {
        return Err(WorkflowValidationError::DependencyOutputBudgetExceeded {
            node_id: node.id.clone(),
            potential_bytes,
            max_bytes: MAX_WORKFLOW_DEPENDENCY_RESULTS_BYTES,
        });
    }
    Ok(())
}

fn validate_dependencies(
    spec: &WorkflowSpec,
    nodes: &BTreeMap<&WorkflowNodeId, &WorkflowAgentNode>,
) -> Result<(), WorkflowValidationError> {
    for node in &spec.nodes {
        let mut seen = BTreeSet::new();
        for dependency in &node.dependencies {
            if dependency == &node.id {
                return Err(WorkflowValidationError::SelfDependency {
                    node_id: node.id.clone(),
                });
            }
            if !valid_workflow_id(dependency.as_str()) {
                return Err(WorkflowValidationError::InvalidNodeId {
                    node_id: dependency.clone(),
                });
            }
            if !seen.insert(dependency) {
                return Err(WorkflowValidationError::DuplicateDependency {
                    node_id: node.id.clone(),
                    dependency: dependency.clone(),
                });
            }
            if !nodes.contains_key(dependency) {
                return Err(WorkflowValidationError::MissingDependency {
                    node_id: node.id.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    Ok(())
}
