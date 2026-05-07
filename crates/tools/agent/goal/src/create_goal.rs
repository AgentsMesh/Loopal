use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::errors::format_session_error;
use crate::get_goal::render_response;

pub struct CreateGoalTool;

#[derive(Deserialize)]
struct CreateGoalArgs {
    objective: String,
    #[serde(default)]
    token_budget: Option<u64>,
}

#[async_trait]
impl Tool for CreateGoalTool {
    fn name(&self) -> &str {
        "create_goal"
    }

    fn description(&self) -> &str {
        "Create a goal only when explicitly requested by the user or system/developer \
         instructions; do not infer goals from ordinary tasks. Set token_budget only when an \
         explicit token budget is requested. Fails if a goal already exists; use update_goal \
         only to mark an existing goal complete."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["objective"],
            "properties": {
                "objective": {
                    "type": "string",
                    "description": "Required. The concrete objective to start pursuing. Must be non-empty."
                },
                "token_budget": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Optional positive token budget for the new active goal."
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
        let args: CreateGoalArgs = match serde_json::from_value(input) {
            Ok(a) => a,
            Err(e) => return Ok(ToolResult::error(format!("invalid arguments: {e}"))),
        };
        if args.objective.trim().is_empty() {
            return Ok(ToolResult::error("objective must be a non-empty string"));
        }
        match session.create(args.objective, args.token_budget).await {
            Ok(goal) => Ok(ToolResult::success(render_response(&Some(goal)))),
            Err(err) => Ok(ToolResult::error(format_session_error(err))),
        }
    }
}
