use loopal_protocol::{
    PermissionAuditDecision, PermissionAuditSource, PermissionDecisionAuditRequest,
    PermissionIntent,
};
use loopal_tool_api::PermissionMode;

use super::permission_request::PermissionRequest;
use super::types::PermissionFastPath;
use crate::hub::Hub;
use crate::types::{AgentExecutionRef, AgentOrigin};

const POLICY_BINDING_GENERATION: u64 = u64::MAX;

#[cfg(test)]
#[path = "permission_fast_path_tests.rs"]
mod tests;

pub(super) fn authorize(
    hub: &Hub,
    request: &PermissionRequest,
    execution: &AgentExecutionRef,
) -> Option<Result<PermissionFastPath, String>> {
    let seed = request.intent_seed()?;
    let source = if hub.has_permission_grant(execution, seed) {
        PermissionAuditSource::RememberedGrant
    } else if policy_workflow_authorized(hub, seed, execution) {
        PermissionAuditSource::Policy
    } else {
        return None;
    };
    let token_prefix = match source {
        PermissionAuditSource::Policy => "policy",
        PermissionAuditSource::RememberedGrant => "grant",
        _ => unreachable!("only Hub-owned fast paths reach receipt issuance"),
    };
    let intent = PermissionIntent::bind(
        seed.clone(),
        execution.connection_generation,
        match source {
            PermissionAuditSource::Policy => POLICY_BINDING_GENERATION,
            _ => hub.ui.capability_snapshot().generation.max(1),
        },
        format!("{token_prefix}:{}", uuid::Uuid::new_v4().simple()),
    )
    .map_err(|error| error.to_string());
    Some(match intent {
        Ok(intent) => PermissionDecisionAuditRequest::from_seed(
            &request.logical_id,
            seed,
            Some(intent.intent_digest()),
            PermissionAuditDecision::Allow,
            source,
        )
        .map(|audit| PermissionFastPath::Authorize {
            audit,
            intent: Box::new(intent),
            source,
        })
        .map_err(|error| error.to_string()),
        Err(error) => Err(error),
    })
}

pub(super) fn authority_is_current(
    hub: &Hub,
    intent: &PermissionIntent,
    execution: &AgentExecutionRef,
    source: PermissionAuditSource,
) -> bool {
    match source {
        PermissionAuditSource::RememberedGrant => {
            hub.has_permission_grant(execution, intent.seed())
        }
        PermissionAuditSource::Policy => policy_workflow_authorized(hub, intent.seed(), execution),
        PermissionAuditSource::Frontend | PermissionAuditSource::Ui => false,
    }
}

fn policy_workflow_authorized(
    hub: &Hub,
    seed: &loopal_protocol::PermissionIntentSeed,
    execution: &AgentExecutionRef,
) -> bool {
    let Some(workflow) = seed.workflow() else {
        return false;
    };
    let Some(facts) = hub.registry.runtime_facts(execution) else {
        return false;
    };
    let Some(parent) = facts.parent.as_ref() else {
        return false;
    };
    facts.origin == AgentOrigin::ManagedChild
        && facts.depth > 0
        && facts.spawn.permission_mode == PermissionMode::Bypass
        && facts.workflow_permission_causation.as_ref() == Some(workflow)
        && facts.workflow_attempt_capability_digest.is_some()
        && hub.registry.owns_active_lease(parent)
        && hub
            .registry
            .runtime_facts(parent)
            .is_some_and(|parent_facts| {
                facts.root == parent.address.agent
                    && crate::workflow::owner_for_managed_root(parent, parent_facts).is_ok()
            })
}
