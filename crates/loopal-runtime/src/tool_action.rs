use std::sync::Arc;

use loopal_error::{LoopalError, ToolError};
use loopal_kernel::Kernel;
use loopal_protocol::{
    PermissionActionDigest, PermissionIntentRequest, PermissionIntentSeed, PermissionSchemaDigest,
    WorkflowPermissionCausation, calculate_permission_action_digest,
    calculate_permission_display_digest, calculate_permission_schema_digest,
};
use loopal_tool_api::Tool;
use serde_json::Value;

pub struct PreparedToolAction {
    id: String,
    tool_name: String,
    placeholder_input: Value,
    digest: PermissionActionDigest,
    tool_schema_digest: PermissionSchemaDigest,
    tool: Arc<dyn Tool>,
    permission_annotation: Option<String>,
    rewritten: bool,
    permission_receipt: Option<loopal_protocol::PermissionReceipt>,
}

impl PreparedToolAction {
    pub(crate) fn new(
        id: String,
        tool_name: String,
        placeholder_input: Value,
        tool: Arc<dyn Tool>,
        rewritten: bool,
    ) -> Self {
        let digest = action_digest(&id, &tool_name, &placeholder_input);
        let tool_schema_digest = calculate_permission_schema_digest(&tool.parameters_schema());
        Self {
            id,
            tool_name,
            placeholder_input,
            digest,
            tool_schema_digest,
            tool,
            permission_annotation: None,
            rewritten,
            permission_receipt: None,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub fn placeholder_input(&self) -> &Value {
        &self.placeholder_input
    }

    pub(crate) fn tool(&self) -> &Arc<dyn Tool> {
        &self.tool
    }

    pub(crate) fn action_digest(&self) -> PermissionActionDigest {
        self.digest
    }

    pub(crate) fn schema_digest(&self) -> PermissionSchemaDigest {
        self.tool_schema_digest
    }

    pub(crate) fn protected_effect_audit_request(
        &self,
    ) -> loopal_error::Result<loopal_protocol::ProtectedEffectAuditRequest> {
        let request = loopal_protocol::ProtectedEffectAuditRequest::new(
            self.id.clone(),
            self.tool_name.clone(),
            self.digest,
            self.tool_schema_digest,
        )
        .map_err(|error| LoopalError::Other(error.to_string()))?;
        Ok(match &self.permission_receipt {
            Some(receipt) => request.with_receipt(receipt.clone()),
            None => request,
        })
    }

    pub(crate) fn set_permission_receipt(&mut self, receipt: loopal_protocol::PermissionReceipt) {
        self.permission_receipt = Some(receipt);
    }

    pub(crate) fn was_rewritten(&self) -> bool {
        self.rewritten
    }

    pub(crate) fn annotate_permission(&mut self, reason: String) -> loopal_error::Result<()> {
        let object = self.placeholder_input.as_object().ok_or_else(|| {
            LoopalError::Tool(ToolError::InvalidInput(
                "sandbox permission annotation requires object input".into(),
            ))
        })?;
        if object.contains_key("sandbox_approval_reason") {
            return Err(LoopalError::Tool(ToolError::InvalidInput(
                "tool input collides with reserved sandbox permission annotation".into(),
            )));
        }
        self.permission_annotation = Some(reason);
        Ok(())
    }

    pub(crate) fn permission_request(
        &self,
        workflow: Option<WorkflowPermissionCausation>,
    ) -> loopal_error::Result<PermissionIntentRequest> {
        let display_input = self.permission_input();
        let seed = PermissionIntentSeed::new(
            self.tool_name.clone(),
            self.digest,
            calculate_permission_display_digest(&display_input),
            self.tool_schema_digest,
            workflow,
        )
        .map_err(invalid_permission_intent)?;
        PermissionIntentRequest::new(
            self.id.clone(),
            self.tool_name.clone(),
            self.placeholder_input.clone(),
            display_input,
            self.tool.parameters_schema(),
            seed,
        )
        .map_err(invalid_permission_intent)
    }

    pub(crate) fn verify(&self, kernel: &Kernel) -> loopal_error::Result<()> {
        let current_tool = kernel.get_tool(&self.tool_name);
        let valid = self.tool.name() == self.tool_name
            && action_digest(&self.id, &self.tool_name, &self.placeholder_input) == self.digest
            && calculate_permission_schema_digest(&self.tool.parameters_schema())
                == self.tool_schema_digest
            && current_tool.as_ref().is_some_and(|tool| {
                Arc::ptr_eq(tool, &self.tool)
                    && calculate_permission_schema_digest(&tool.parameters_schema())
                        == self.tool_schema_digest
            });
        if valid {
            Ok(())
        } else {
            Err(LoopalError::Tool(ToolError::InvalidInput(
                "approved tool action integrity mismatch".into(),
            )))
        }
    }

    fn permission_input(&self) -> Value {
        let mut input = self.placeholder_input.clone();
        if let Some(reason) = &self.permission_annotation {
            input
                .as_object_mut()
                .expect("annotation input validated")
                .insert(
                    "sandbox_approval_reason".into(),
                    Value::String(reason.clone()),
                );
        }
        input
    }
}

fn invalid_permission_intent(error: impl std::fmt::Display) -> LoopalError {
    LoopalError::Tool(ToolError::InvalidInput(format!(
        "permission intent construction failed: {error}"
    )))
}

fn action_digest(id: &str, tool_name: &str, input: &Value) -> PermissionActionDigest {
    calculate_permission_action_digest(id, tool_name, input)
}

#[cfg(test)]
mod tests;
