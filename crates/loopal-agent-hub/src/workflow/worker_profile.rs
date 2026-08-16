use loopal_protocol::{WorkflowNodeId, WorkflowSpec, WorkflowWorkerProfileRef};

use super::WorkflowCoordinatorError;

/// Hub-owned worker policy selected by an untrusted workflow profile reference.
///
/// Keep this closed in V1: an untrusted profile reference cannot carry runtime
/// overrides. Each allowlisted profile maps to a Hub-owned authority ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedWorkflowWorkerProfile {
    Default,
    Explore,
    Plan,
}

impl ResolvedWorkflowWorkerProfile {
    pub(crate) fn resolve(
        profile: &WorkflowWorkerProfileRef,
    ) -> Result<Self, WorkflowCoordinatorError> {
        match profile.as_str() {
            "default" => Ok(Self::Default),
            "explore" => Ok(Self::Explore),
            "plan" => Ok(Self::Plan),
            _ => Err(WorkflowCoordinatorError::UnsupportedWorkerProfile {
                profile: profile.clone(),
            }),
        }
    }

    pub(crate) const fn agent_type(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Explore => "explore",
            Self::Plan => "plan",
        }
    }

    pub(crate) fn intersect_authority(
        self,
        mut root: crate::types::SpawnAuthority,
    ) -> crate::types::SpawnAuthority {
        if matches!(self, Self::Explore | Self::Plan) {
            root.sandbox_policy = loopal_config::SandboxPolicy::ReadOnly;
        }
        root
    }
}

pub(crate) fn validate_spec_profiles(spec: &WorkflowSpec) -> Result<(), WorkflowCoordinatorError> {
    for node in &spec.nodes {
        resolve_node_profile(&node.id, &node.worker_profile)?;
    }
    Ok(())
}

pub(crate) fn resolve_node_profile(
    node_id: &WorkflowNodeId,
    profile: &WorkflowWorkerProfileRef,
) -> Result<ResolvedWorkflowWorkerProfile, WorkflowCoordinatorError> {
    ResolvedWorkflowWorkerProfile::resolve(profile).map_err(|error| match error {
        WorkflowCoordinatorError::UnsupportedWorkerProfile { profile } => {
            WorkflowCoordinatorError::UnsupportedWorkerProfileForNode {
                node_id: node_id.clone(),
                profile,
            }
        }
        other => other,
    })
}
