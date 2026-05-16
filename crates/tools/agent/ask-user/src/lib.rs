use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolDispatch, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct AskUserTool;

#[derive(Deserialize, JsonSchema)]
pub struct AskUserOption {
    pub label: String,
    pub description: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct AskUserQuestion {
    pub question: String,
    #[serde(default)]
    pub header: Option<String>,
    pub options: Vec<AskUserOption>,
    #[serde(default)]
    #[serde(rename = "multiSelect")]
    pub multi_select: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AskUserParams {
    pub questions: Vec<AskUserQuestion>,
}

#[async_trait]
impl TypedTool<AskUserParams> for AskUserTool {
    fn name(&self) -> &str {
        "AskUser"
    }

    fn description(&self) -> &str {
        "Present one or more questions to the user with predefined options.\n\
         Use when you need clarification, a decision, or user preferences.\n\
         - Users can always select 'Other' to provide custom text input.\n\
         - Use multiSelect: true when choices are not mutually exclusive.\n\
         - In plan mode: use this to clarify requirements BEFORE finalizing the plan. \
         Do NOT use it to ask 'Is my plan ready?' — use ExitPlanMode for that.\n\
         \n\
         Tool result format: per-question lines `Q{i} ({question}): {answer}` \
         joined by newline. Special tokens: `(cancelled by user)`, \
         `(no selection)`, `(unsupported: ...)` mean the user did not provide \
         a real answer."
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
        _input: AskUserParams,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        Ok(ToolResult::success("(intercepted by runner)"))
    }
}
