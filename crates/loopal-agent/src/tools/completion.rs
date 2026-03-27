use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};

/// Tool for agents to signal task completion.
///
/// Returns a `ToolResult::completion(result)` that the runner detects
/// via `is_completion: true` to exit the turn loop.
pub struct AttemptCompletionTool;

#[async_trait]
impl Tool for AttemptCompletionTool {
    fn name(&self) -> &str {
        "AttemptCompletion"
    }

    fn description(&self) -> &str {
        "Signal that you have completed the assigned task. \
         Provide a result summary in the 'result' field. \
         This will terminate the current agent."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "result": {
                    "type": "string",
                    "description": "Summary of the completed work"
                }
            },
            "required": ["result"]
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let result = input
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("Task completed.");

        Ok(ToolResult::completion(result))
    }
}
