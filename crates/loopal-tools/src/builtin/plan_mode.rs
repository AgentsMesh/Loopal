use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// EnterPlanMode
// ---------------------------------------------------------------------------

pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "EnterPlanMode"
    }

    fn description(&self) -> &str {
        "Switch the agent into plan mode. In plan mode only read-only tools are \
         available, allowing safe exploration and planning before making changes."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        // Intercepted by the agent loop runner before reaching here.
        Ok(ToolResult::success(
            "Entered plan mode (intercepted by runner)",
        ))
    }
}

// ---------------------------------------------------------------------------
// ExitPlanMode
// ---------------------------------------------------------------------------

pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "ExitPlanMode"
    }

    fn description(&self) -> &str {
        "Exit plan mode and return to the normal mode where all tools are available."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        // Intercepted by the agent loop runner before reaching here.
        Ok(ToolResult::success(
            "Exited plan mode (intercepted by runner)",
        ))
    }
}
