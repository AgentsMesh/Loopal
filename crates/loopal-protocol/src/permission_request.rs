use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::PermissionIntentSeed;
use crate::permission_action::{
    calculate_permission_action_digest, calculate_permission_display_digest,
    calculate_permission_schema_digest,
};

const MAX_TOOL_CALL_ID_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionIntentRequest {
    pub tool_call_id: String,
    pub tool_name: String,
    pub action_input: Value,
    #[serde(rename = "tool_input")]
    pub display_input: Value,
    pub tool_schema: Value,
    #[serde(rename = "permission_intent")]
    pub intent_seed: PermissionIntentSeed,
}

impl PermissionIntentRequest {
    pub fn create(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        action_input: Value,
        display_input: Value,
        tool_schema: Value,
        workflow: Option<crate::WorkflowPermissionCausation>,
    ) -> Result<Self, PermissionRequestError> {
        let tool_call_id = tool_call_id.into();
        let tool_name = tool_name.into();
        let intent_seed = PermissionIntentSeed::new(
            tool_name.clone(),
            calculate_permission_action_digest(&tool_call_id, &tool_name, &action_input),
            calculate_permission_display_digest(&display_input),
            calculate_permission_schema_digest(&tool_schema),
            workflow,
        )
        .map_err(|_| PermissionRequestError::ToolName)?;
        Self::new(
            tool_call_id,
            tool_name,
            action_input,
            display_input,
            tool_schema,
            intent_seed,
        )
    }

    pub fn new(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        action_input: Value,
        display_input: Value,
        tool_schema: Value,
        intent_seed: PermissionIntentSeed,
    ) -> Result<Self, PermissionRequestError> {
        let request = Self {
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            action_input,
            display_input,
            tool_schema,
            intent_seed,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), PermissionRequestError> {
        if self.tool_call_id.is_empty()
            || self.tool_call_id.len() > MAX_TOOL_CALL_ID_BYTES
            || self.tool_call_id.chars().any(char::is_control)
        {
            return Err(PermissionRequestError::ToolCallId);
        }
        if self.intent_seed.tool_name() != self.tool_name {
            return Err(PermissionRequestError::ToolName);
        }
        if !valid_display(&self.action_input, &self.display_input) {
            return Err(PermissionRequestError::DisplayInput);
        }
        if calculate_permission_action_digest(
            &self.tool_call_id,
            &self.tool_name,
            &self.action_input,
        ) != self.intent_seed.action_digest()
        {
            return Err(PermissionRequestError::ActionDigest);
        }
        if calculate_permission_display_digest(&self.display_input)
            != self.intent_seed.display_digest()
        {
            return Err(PermissionRequestError::DisplayDigest);
        }
        if calculate_permission_schema_digest(&self.tool_schema) != self.intent_seed.schema_digest()
        {
            return Err(PermissionRequestError::SchemaDigest);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionRequestError {
    ToolCallId,
    ToolName,
    DisplayInput,
    ActionDigest,
    DisplayDigest,
    SchemaDigest,
}

impl fmt::Display for PermissionRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ToolCallId => "invalid permission tool call id",
            Self::ToolName => "permission intent tool name mismatch",
            Self::DisplayInput => "permission display input is not derived from the action",
            Self::ActionDigest => "permission action digest mismatch",
            Self::DisplayDigest => "permission display digest mismatch",
            Self::SchemaDigest => "permission schema digest mismatch",
        })
    }
}

impl std::error::Error for PermissionRequestError {}

fn valid_display(action: &Value, display: &Value) -> bool {
    if action == display {
        return !action
            .as_object()
            .is_some_and(|object| object.contains_key("sandbox_approval_reason"));
    }
    let (Some(action), Some(display)) = (action.as_object(), display.as_object()) else {
        return false;
    };
    display.len() == action.len().saturating_add(1)
        && action
            .iter()
            .all(|(key, value)| display.get(key) == Some(value))
        && display
            .get("sandbox_approval_reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| !reason.is_empty())
}
