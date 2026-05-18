use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{GoalSessionError, PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::errors::format_session_error;
use crate::get_goal::render_response;

pub struct UpdateGoalTool;

#[derive(Deserialize, JsonSchema, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatusInput {
    Active,
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
        "Update the existing goal. Use `complete` only when the objective has actually been \
         achieved and no required work remains. Use `active` only to revert a mistaken \
         `complete` — you previously marked the goal complete and then discovered remaining \
         work that still falls under the same original objective; reopening preserves the \
         goal's id and objective. To pursue a different objective, use create_goal instead. \
         Do not mark complete merely because you are stopping work, and do not toggle between \
         `active` and `complete` to manipulate continuation. You cannot pause or resume the \
         goal via this tool; those status changes are controlled by the user or system."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
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
        let outcome: Result<_, GoalSessionError> = match input.status {
            GoalStatusInput::Complete => session.complete_by_model().await,
            GoalStatusInput::Active => session.reopen_by_model().await,
        };
        match outcome {
            Ok(goal) => Ok(ToolResult::success(render_response(&Some(goal)))),
            Err(err) => Ok(ToolResult::error(format_session_error(err))),
        }
    }
}
