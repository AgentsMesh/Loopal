use serde::Serialize;
use serde::de::DeserializeOwned;

use loopal_error::LoopalError;
use loopal_tool_api::{ToolContext, ToolResult};

use crate::shared::AgentShared;
use crate::workflow_control::WorkflowControlClient;

pub(super) fn client(
    ctx: &ToolContext,
) -> Result<std::sync::Arc<dyn WorkflowControlClient>, LoopalError> {
    let shared = ctx
        .shared
        .as_ref()
        .and_then(|value| value.downcast_ref::<std::sync::Arc<AgentShared>>())
        .ok_or_else(|| invalid("AgentShared is unavailable"))?;
    shared
        .workflow_control
        .clone()
        .ok_or_else(|| invalid("workflow execution is disabled for this agent"))
}

pub(super) fn decode<T: DeserializeOwned>(input: serde_json::Value) -> Result<T, LoopalError> {
    serde_json::from_value(input).map_err(|error| invalid(&error.to_string()))
}

pub(super) fn result<T: Serialize>(response: Result<T, String>) -> ToolResult {
    match response {
        Ok(value) => match serde_json::to_string_pretty(&value) {
            Ok(value) => ToolResult::success(value),
            Err(error) => ToolResult::error(format!("workflow response encoding failed: {error}")),
        },
        Err(error) => ToolResult::error(error),
    }
}

pub(super) fn start_result<T: Serialize>(
    response: Result<T, crate::workflow_control::WorkflowStartControlError>,
) -> ToolResult {
    use crate::workflow_control::WorkflowStartControlError;

    match response {
        Ok(value) => result(Ok(value)),
        Err(WorkflowStartControlError::Rejected(message)) => ToolResult::error(
            serde_json::json!({
                "outcome": "rejected",
                "fallback": "direct",
                "message": message,
            })
            .to_string(),
        ),
        Err(WorkflowStartControlError::Indeterminate {
            request_id,
            message,
        }) => ToolResult::error(
            serde_json::json!({
                "outcome": "indeterminate",
                "request_id": request_id,
                "message": message,
            })
            .to_string(),
        ),
    }
}

fn invalid(message: &str) -> LoopalError {
    LoopalError::Tool(loopal_error::ToolError::InvalidInput(message.into()))
}
