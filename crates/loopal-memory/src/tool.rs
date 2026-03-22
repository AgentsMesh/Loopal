use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

pub struct MemoryTool;

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "Memory"
    }

    fn description(&self) -> &str {
        "Record a stable observation for cross-session memory. \
         Use when: user corrects your behavior or states a preference, \
         you discover a non-obvious project convention or architecture decision reason, \
         you encounter a recurring issue and its solution, \
         or the user explicitly asks you to remember something. \
         Do NOT record: info inferable from code, temporary task details, file structure."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["observation"],
            "properties": {
                "observation": {
                    "type": "string",
                    "description": "The stable knowledge to remember across sessions"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let observation = input["observation"]
            .as_str()
            .ok_or_else(|| {
                LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                    "observation is required".into(),
                ))
            })?;

        if observation.trim().is_empty() {
            return Ok(ToolResult::error("observation must not be empty"));
        }

        match &ctx.memory_channel {
            Some(ch) => match ch.try_send(observation.to_string()) {
                Ok(()) => Ok(ToolResult::success("Noted.")),
                Err(e) => Ok(ToolResult::error(format!("Memory channel full: {e}"))),
            },
            None => Ok(ToolResult::error("Memory is not enabled")),
        }
    }
}
