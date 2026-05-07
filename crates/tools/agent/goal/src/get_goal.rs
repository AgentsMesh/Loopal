use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_protocol::ThreadGoal;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde::Serialize;
use serde_json::{Value, json};

use crate::errors::format_session_error;

pub struct GetGoalTool;

#[async_trait]
impl Tool for GetGoalTool {
    fn name(&self) -> &str {
        "get_goal"
    }

    fn description(&self) -> &str {
        "Get the current goal for this thread, including status, budgets, token and elapsed-time \
         usage, and remaining token budget. Returns null when no goal is set."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let session = match ctx.goal_session.as_ref() {
            Some(s) => s,
            None => {
                return Ok(ToolResult::error(
                    "goal feature is disabled in this session",
                ));
            }
        };
        match session.snapshot().await {
            Ok(goal) => Ok(ToolResult::success(render_response(&goal))),
            Err(err) => Ok(ToolResult::error(format_session_error(err))),
        }
    }
}

#[derive(Serialize)]
struct GoalResponse<'a> {
    goal: Option<&'a ThreadGoal>,
    remaining_tokens: Option<u64>,
}

pub(crate) fn render_response(goal: &Option<ThreadGoal>) -> String {
    let body = GoalResponse {
        goal: goal.as_ref(),
        remaining_tokens: goal.as_ref().and_then(|g| g.remaining_tokens()),
    };
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_string())
}
