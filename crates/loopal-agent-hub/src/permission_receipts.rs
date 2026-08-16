use std::collections::HashMap;

use loopal_protocol::{
    PermissionActionDigest, PermissionIntent, PermissionIntentDigest, PermissionReceipt,
    PermissionReceiptError, PermissionSchemaDigest, WorkflowPermissionCausation,
};

use crate::types::AgentExecutionRef;

#[derive(Clone, Debug)]
struct Issuance {
    execution: AgentExecutionRef,
    action_digest: PermissionActionDigest,
    schema_digest: PermissionSchemaDigest,
    intent_digest: PermissionIntentDigest,
    execution_generation: u64,
    ui_generation: u64,
    workflow: Option<WorkflowPermissionCausation>,
    require_current_ui: bool,
    consumed: bool,
}

/// Hub-owned receipt issuance ledger. The wire receipt is only a capability;
/// the ledger is the authority that makes issuance single-use and binds it to
/// the authenticated Agent lease.
#[derive(Default)]
pub(crate) struct PermissionReceiptRegistry {
    issuances: HashMap<String, Issuance>,
}

impl PermissionReceiptRegistry {
    pub(crate) fn issue(
        &mut self,
        intent: &PermissionIntent,
        execution: &AgentExecutionRef,
        require_current_ui: bool,
    ) -> Result<PermissionReceipt, PermissionReceiptError> {
        let issuance = format!("hub:{}", uuid::Uuid::new_v4().simple());
        let receipt = PermissionReceipt::issue_for_intent(intent, issuance.clone())?;
        let seed = intent.seed();
        self.issuances.insert(
            issuance,
            Issuance {
                execution: execution.clone(),
                action_digest: seed.action_digest(),
                schema_digest: seed.schema_digest(),
                intent_digest: intent.intent_digest(),
                execution_generation: intent.execution_generation(),
                ui_generation: intent.ui_generation(),
                workflow: seed.workflow().cloned(),
                require_current_ui,
                consumed: false,
            },
        );
        Ok(receipt)
    }

    pub(crate) fn consume(
        &mut self,
        receipt: &PermissionReceipt,
        action_digest: PermissionActionDigest,
        schema_digest: PermissionSchemaDigest,
        execution: &AgentExecutionRef,
        workflow: Option<&WorkflowPermissionCausation>,
        current_ui_generation: u64,
    ) -> Result<(), String> {
        receipt
            .validate_effect_binding(
                action_digest,
                schema_digest,
                execution.connection_generation,
                workflow,
            )
            .map_err(|error| error.to_string())?;
        let issuance = self
            .issuances
            .get_mut(receipt.audit_issuance())
            .ok_or_else(|| "permission receipt issuance is unknown".to_string())?;
        if issuance.consumed {
            return Err("permission receipt was already consumed".into());
        }
        if issuance.action_digest != action_digest
            || issuance.schema_digest != schema_digest
            || issuance.intent_digest != receipt.intent_digest()
            || issuance.execution != *execution
            || issuance.execution_generation != execution.connection_generation
            || issuance.ui_generation != receipt.ui_generation()
            || issuance.workflow.as_ref() != workflow
            || (issuance.require_current_ui && issuance.ui_generation != current_ui_generation)
        {
            return Err("permission receipt issuance binding mismatch".into());
        }
        issuance.consumed = true;
        Ok(())
    }

    pub(crate) fn revoke_execution(&mut self, execution: &AgentExecutionRef) {
        self.issuances
            .retain(|_, issuance| issuance.execution != *execution);
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.issuances.len()
    }
}

#[cfg(test)]
#[path = "permission_receipts_tests.rs"]
mod tests;
