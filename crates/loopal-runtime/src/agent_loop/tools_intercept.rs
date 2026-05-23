use loopal_error::Result;
use loopal_provider_api::ContentBlock;
use loopal_tool_ask_user::NAME as ASK_USER_NAME;
use loopal_tool_idle::NAME as REQUEST_IDLE_NAME;
use loopal_tool_plan_mode::{ENTER_PLAN_NAME, EXIT_PLAN_NAME};

use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn intercept_special_tools(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<(
        Vec<(usize, ContentBlock)>,
        Vec<(String, String, serde_json::Value)>,
    )> {
        let mut intercepted = Vec::new();
        let mut remaining = Vec::new();

        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            let entry = match name.as_str() {
                n if n == ENTER_PLAN_NAME => self.handle_enter_plan(turn_ctx, idx, id).await?,
                n if n == EXIT_PLAN_NAME => self.handle_exit_plan(turn_ctx, idx, id).await?,
                n if n == ASK_USER_NAME => {
                    self.handle_ask_user(turn_ctx, idx, id, name, input).await?
                }
                n if n == REQUEST_IDLE_NAME => {
                    self.handle_request_idle(turn_ctx, idx, id, input).await?
                }
                _ => {
                    remaining.push((id.clone(), name.clone(), input.clone()));
                    continue;
                }
            };
            intercepted.push(entry);
        }
        Ok((intercepted, remaining))
    }
}
