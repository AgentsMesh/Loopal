use loopal_error::Result;
use loopal_message::ContentBlock;

use super::runner::AgentLoopRunner;

pub(super) type Intercepted = Vec<(usize, ContentBlock)>;
pub(super) type Remaining = Vec<(String, String, serde_json::Value)>;

impl AgentLoopRunner {
    pub(super) async fn intercept_special_tools(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<(Intercepted, Remaining)> {
        let mut intercepted = Vec::new();
        let mut remaining = Vec::new();

        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            match name.as_str() {
                "EnterPlanMode" => intercepted.push(self.handle_enter_plan(idx, id).await?),
                "ExitPlanMode" => intercepted.push(self.handle_exit_plan(idx, id).await?),
                "AskUser" => intercepted.push(self.handle_ask_user(idx, id, name, input).await?),
                _ => remaining.push((id.clone(), name.clone(), input.clone())),
            }
        }
        Ok((intercepted, remaining))
    }
}
