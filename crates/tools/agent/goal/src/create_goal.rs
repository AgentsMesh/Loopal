use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::errors::format_session_error;
use crate::get_goal::render_response;

pub struct CreateGoalTool;

#[derive(Deserialize, JsonSchema)]
pub struct CreateGoalParams {
    pub objective: String,
    #[serde(default)]
    pub token_budget: Option<u64>,
}

#[async_trait]
impl TypedTool<CreateGoalParams> for CreateGoalTool {
    fn name(&self) -> &str {
        "create_goal"
    }

    fn description(&self) -> &str {
        "Create a goal only when explicitly requested by the user or system/developer \
         instructions; do not infer goals from ordinary tasks. Set token_budget only when an \
         explicit token budget is requested. Fails if a goal already exists; use update_goal \
         only to mark an existing goal complete."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(
        &self,
        input: CreateGoalParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let session = match ctx.goal_session.as_ref() {
            Some(s) => s,
            None => {
                return Ok(ToolResult::error(
                    "goal feature is disabled in this session",
                ));
            }
        };
        if input.objective.trim().is_empty() {
            return Ok(ToolResult::error("objective must be a non-empty string"));
        }
        match session.create(input.objective, input.token_budget).await {
            Ok(goal) => Ok(ToolResult::success(render_response(&Some(goal)))),
            Err(err) => Ok(ToolResult::error(format_session_error(err))),
        }
    }
}
