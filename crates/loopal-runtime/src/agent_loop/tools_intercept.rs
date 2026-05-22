use loopal_error::Result;
use loopal_message::ContentBlock;
use loopal_tool_idle::NAME as REQUEST_IDLE_NAME;
use loopal_tool_plan_mode::{ENTER_PLAN_NAME, EXIT_PLAN_NAME};

use super::runner::AgentLoopRunner;

pub(super) type Intercepted = Vec<(usize, ContentBlock)>;
pub(super) type Remaining = Vec<(String, String, serde_json::Value)>;

impl AgentLoopRunner {
    pub(super) async fn intercept_special_tools(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<(Intercepted, Remaining, bool)> {
        let mut intercepted = Vec::new();
        let mut remaining = Vec::new();
        let mut turn_end_signal = false;

        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            let outcome = match name.as_str() {
                n if n == ENTER_PLAN_NAME => Some(self.handle_enter_plan(idx, id).await?),
                n if n == EXIT_PLAN_NAME => Some(self.handle_exit_plan(idx, id).await?),
                "AskUser" => Some(self.handle_ask_user(idx, id, name, input).await?),
                n if n == REQUEST_IDLE_NAME => {
                    Some(self.handle_request_idle(idx, id, input).await?)
                }
                _ => {
                    remaining.push((id.clone(), name.clone(), input.clone()));
                    None
                }
            };
            if let Some((i, block, signal)) = outcome {
                intercepted.push((i, block));
                turn_end_signal |= signal;
            }
        }
        Ok((intercepted, remaining, turn_end_signal))
    }
}
