use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::errors::format_session_error;
use crate::get_goal::render_response;

pub struct UpdateGoalTool;

#[derive(Deserialize, JsonSchema, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatusInput {
    Complete,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateGoalParams {
    pub status: GoalStatusInput,
}

#[async_trait]
impl TypedTool<UpdateGoalParams> for UpdateGoalTool {
    fn name(&self) -> &str {
        "update_goal"
    }

    fn description(&self) -> &str {
        "Update the existing goal. Use this tool only to mark the goal achieved. Set status to \
         `complete` only when the objective has actually been achieved and no required work \
         remains. Do not mark a goal complete merely because its budget is nearly exhausted or \
         because you are stopping work. You cannot use this tool to pause, resume or \
         budget-limit a goal; those status changes are controlled by the user or system."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(
        &self,
        input: UpdateGoalParams,
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
        let _ = input.status;
        match session.complete_by_model().await {
            Ok(goal) => Ok(ToolResult::success(render_response(&Some(goal)))),
            Err(err) => Ok(ToolResult::error(format_session_error(err))),
        }
    }
}
