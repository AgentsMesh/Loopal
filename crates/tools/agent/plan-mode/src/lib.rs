use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolDispatch, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub const ENTER_PLAN_NAME: &str = "EnterPlanMode";
pub const EXIT_PLAN_NAME: &str = "ExitPlanMode";

pub struct EnterPlanModeTool;

#[derive(Deserialize, JsonSchema)]
pub struct EnterPlanModeParams {}

#[async_trait]
impl TypedTool<EnterPlanModeParams> for EnterPlanModeTool {
    fn name(&self) -> &str {
        ENTER_PLAN_NAME
    }

    fn description(&self) -> &str {
        "Use this tool proactively before starting non-trivial implementation tasks.\n\
         When to use: new features, multiple valid approaches, code modifications affecting existing behavior, \
         architectural decisions, multi-file changes, unclear requirements.\n\
         When NOT to use: single-line fixes, trivial bugs, small tweaks, or when the user gave very specific instructions.\n\
         In plan mode, only read-only tools are available for safe exploration and planning."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    fn dispatch(&self) -> ToolDispatch {
        ToolDispatch::RunnerDirect
    }

    async fn execute(
        &self,
        _input: EnterPlanModeParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success(
            "Entered plan mode (intercepted by runner)",
        ))
    }
}

pub struct ExitPlanModeTool;

#[derive(Deserialize, JsonSchema)]
pub struct ExitPlanModeParams {}

#[async_trait]
impl TypedTool<ExitPlanModeParams> for ExitPlanModeTool {
    fn name(&self) -> &str {
        EXIT_PLAN_NAME
    }

    fn description(&self) -> &str {
        "Exit plan mode and return to normal mode where all tools are available.\n\
         This tool reads the plan from the plan file you wrote — it does not take the plan content as a parameter.\n\
         Only use for implementation planning, not for research or exploration tasks.\n\
         Do NOT use AskUserQuestion to ask 'Is this plan okay?' — use ExitPlanMode instead, which inherently requests approval."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    fn dispatch(&self) -> ToolDispatch {
        ToolDispatch::RunnerDirect
    }

    async fn execute(
        &self,
        _input: ExitPlanModeParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success(
            "Exited plan mode (intercepted by runner)",
        ))
    }
}
