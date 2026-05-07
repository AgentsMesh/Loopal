use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::errors::format_session_error;
use crate::get_goal::render_response;

pub struct UpdateGoalTool;

#[derive(Deserialize)]
struct UpdateGoalArgs {
    status: ModelStatusInput,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ModelStatusInput {
    Complete,
}

#[async_trait]
impl Tool for UpdateGoalTool {
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

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["status"],
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["complete"],
                    "description": "Required. Only `complete` is permitted; the model cannot pause or budget-limit a goal."
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let session = match ctx.goal_session.as_ref() {
            Some(s) => s,
            None => {
                return Ok(ToolResult::error(
                    "goal feature is disabled in this session",
                ));
            }
        };
        let args: UpdateGoalArgs = match serde_json::from_value(input) {
            Ok(a) => a,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "invalid arguments: {e}; status must be \"complete\""
                )));
            }
        };
        let _ = args.status;
        match session.complete_by_model().await {
            Ok(goal) => Ok(ToolResult::success(render_response(&Some(goal)))),
            Err(err) => Ok(ToolResult::error(format_session_error(err))),
        }
    }
}
