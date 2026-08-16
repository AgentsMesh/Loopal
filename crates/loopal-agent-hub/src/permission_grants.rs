use loopal_protocol::{PermissionIntentSeed, PermissionSchemaDigest};

use crate::Hub;
use crate::types::AgentExecutionRef;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PermissionGrantKey {
    execution: AgentExecutionRef,
    tool_name: String,
    schema_digest: PermissionSchemaDigest,
}

impl PermissionGrantKey {
    fn from_seed(execution: AgentExecutionRef, seed: &PermissionIntentSeed) -> Option<Self> {
        seed.workflow().is_none().then(|| Self {
            execution,
            tool_name: seed.tool_name().to_string(),
            schema_digest: seed.schema_digest(),
        })
    }
}

impl Hub {
    pub(crate) fn has_permission_grant(
        &self,
        execution: &AgentExecutionRef,
        seed: &PermissionIntentSeed,
    ) -> bool {
        PermissionGrantKey::from_seed(execution.clone(), seed)
            .is_some_and(|key| self.session_permission_grants.contains(&key))
    }

    pub(crate) fn grant_permission(
        &mut self,
        execution: AgentExecutionRef,
        seed: &PermissionIntentSeed,
    ) -> bool {
        PermissionGrantKey::from_seed(execution, seed)
            .is_some_and(|key| self.session_permission_grants.insert(key))
    }

    pub(crate) fn clear_permission_grants(&mut self, execution: &AgentExecutionRef) {
        self.session_permission_grants
            .retain(|grant| &grant.execution != execution);
    }

    pub(crate) fn clear_permission_grants_for_agent(&mut self, agent_name: &str) {
        self.session_permission_grants
            .retain(|grant| grant.execution.address.agent != agent_name);
    }
}
