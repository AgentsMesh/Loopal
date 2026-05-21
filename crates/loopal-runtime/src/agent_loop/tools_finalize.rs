use loopal_error::Result;
use loopal_message::{ContentBlock, Message, MessageRole};
use tracing::error;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) fn finalize_tool_results(
        &mut self,
        mut indexed_results: Vec<(usize, ContentBlock)>,
    ) -> Result<()> {
        if indexed_results.is_empty() {
            return Ok(());
        }
        indexed_results.sort_by_key(|(idx, _)| *idx);
        let blocks: Vec<ContentBlock> = indexed_results.into_iter().map(|(_, b)| b).collect();

        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: blocks,
            origin: None,
            ephemeral_in_history: false,
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist message");
        }
        self.params.store.push_tool_results(msg);
        Ok(())
    }
}
