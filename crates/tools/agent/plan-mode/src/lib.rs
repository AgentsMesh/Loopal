use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

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
        "Use this tool proactively before starting non-trivial implementation tasks.\n\
         When to use: new features, multiple valid approaches, code modifications affecting existing behavior, \
         architectural decisions, multi-file changes, unclear requirements.\n\
         When NOT to use: single-line fixes, trivial bugs, small tweaks, or when the user gave very specific instructions.\n\
         In plan mode, only read-only tools are available for safe exploration and planning."
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
        "Exit plan mode and return to normal mode where all tools are available.\n\
         This tool reads the plan from the plan file you wrote — it does not take the plan content as a parameter.\n\
         Only use for implementation planning, not for research or exploration tasks.\n\
         Do NOT use AskUserQuestion to ask 'Is this plan okay?' — use ExitPlanMode instead, which inherently requests approval."
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
