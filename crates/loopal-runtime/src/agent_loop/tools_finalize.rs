use loopal_error::Result;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{ToolExecState, ToolResult};
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
        let item_updates = collect_item_updates(&blocks, self.current_tool_batch_item_ids());

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
        // Domain mirror: patch each item state on the in-flight ToolBatch step
        // (started in execute_tools); ToolCall.name/input remain authoritative.
        // Same precondition as emit_all_interrupted: skip the loop if the
        // batch failed to open, so each update doesn't log NoToolBatchOpen.
        if self.turns.current_tool_batch_step().is_some() {
            for (item_index, new_state) in item_updates {
                self.update_tool_batch_item_state(item_index, new_state);
            }
            self.close_tool_batch_record();
        }
        // reason: dual-write transitional — see ContextStore::refresh_view doc.
        self.params.store.push_tool_results(msg);
        Ok(())
    }

    /// Snapshot of (tool_use_id → item_index) for the in-flight ToolBatch step.
    /// Used to map ToolResult blocks back to the correct item position.
    pub(super) fn current_tool_batch_item_ids(&self) -> Vec<String> {
        let Some(step_index) = self.turns.current_tool_batch_step() else {
            return Vec::new();
        };
        let Some(turn) = self.turns.store().current_turn() else {
            return Vec::new();
        };
        let Some(loopal_turn::TurnStep::ToolBatch(batch)) =
            turn.body.steps.get(step_index as usize)
        else {
            return Vec::new();
        };
        batch
            .items
            .iter()
            .map(|i| i.call.id.as_str().to_string())
            .collect()
    }
}

fn collect_item_updates(
    blocks: &[ContentBlock],
    batch_ids: Vec<String>,
) -> Vec<(u32, ToolExecState)> {
    let mut updates = Vec::with_capacity(batch_ids.len());
    for block in blocks {
        let ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
            images,
            ..
        } = block
        else {
            continue;
        };
        let Some(item_index) = batch_ids.iter().position(|id| id == tool_use_id) else {
            continue;
        };
        updates.push((
            item_index as u32,
            ToolExecState::Done(ToolResult {
                content: content.clone(),
                is_error: *is_error,
                images: images.clone(),
            }),
        ));
    }
    updates
}
