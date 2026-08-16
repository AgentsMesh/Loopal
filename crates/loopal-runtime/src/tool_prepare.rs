use loopal_config::HookEvent;
use loopal_hooks::{HookContext, PermissionOverride};
use loopal_kernel::Kernel;
use serde_json::Value;
use tracing::warn;

use crate::tool_action::PreparedToolAction;
use crate::tool_input_validation::{validate_tool_input, validate_wire_refs};

pub enum ToolPreparation {
    Prepared(Box<PreparedToolAction>),
    Denied(String),
}

impl ToolPreparation {
    pub fn into_prepared(self) -> loopal_error::Result<PreparedToolAction> {
        match self {
            Self::Prepared(action) => Ok(*action),
            Self::Denied(message) => Err(loopal_error::LoopalError::Permission(message)),
        }
    }
}

pub async fn prepare_tool_action(
    kernel: &Kernel,
    id: &str,
    name: &str,
    input: Value,
) -> loopal_error::Result<ToolPreparation> {
    let tool = kernel.get_tool(name).ok_or_else(|| {
        loopal_error::LoopalError::Tool(loopal_error::ToolError::NotFound(name.to_string()))
    })?;
    if input
        .as_object()
        .is_some_and(|object| object.contains_key("sandbox_approval_reason"))
    {
        return Ok(ToolPreparation::Denied(
            "Invalid tool input: tool input collides with reserved sandbox permission annotation"
                .into(),
        ));
    }
    if let Err(reason) = validate_tool_input(tool.as_ref(), &input) {
        return Ok(ToolPreparation::Denied(format!(
            "Invalid tool input: {reason}"
        )));
    }
    if let Err(reason) = validate_wire_refs(&input, tool.secret_eligible_params()) {
        return Ok(ToolPreparation::Denied(format!(
            "Invalid tool input: {reason}"
        )));
    }
    if let Some(reason) = tool.precheck(&input) {
        return Ok(ToolPreparation::Denied(format!("Sandbox: {reason}")));
    }
    let outputs = kernel
        .hook_service()
        .run_hooks(
            HookEvent::PreToolUse,
            &HookContext {
                tool_name: Some(name),
                tool_input: Some(&input),
                ..Default::default()
            },
        )
        .await;

    let mut final_input = input;
    let mut rewritten = false;
    for output in outputs {
        if let Some(PermissionOverride::Deny { reason }) = output.permission {
            warn!(tool = name, %reason, "pre-hook rejected");
            return Ok(ToolPreparation::Denied(format!(
                "Pre-hook rejected: {reason}"
            )));
        }
        if let Some(updated) = output.updated_input {
            if rewritten {
                warn!(
                    tool = name,
                    "multiple pre-hooks modified input, later override wins"
                );
            }
            final_input = updated;
            rewritten = true;
        }
    }

    Ok(ToolPreparation::Prepared(Box::new(
        PreparedToolAction::new(
            id.to_string(),
            name.to_string(),
            final_input,
            tool,
            rewritten,
        ),
    )))
}

#[cfg(test)]
#[path = "tool_prepare/tests.rs"]
mod tests;
