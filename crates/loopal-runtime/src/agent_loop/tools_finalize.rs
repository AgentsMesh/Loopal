use loopal_error::Result;
use loopal_provider_api::ContentBlock;
use loopal_turn::{ToolExecState, ToolResult};

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

        if self.turns.current_tool_batch_step().is_some() {
            for (item_index, new_state) in item_updates {
                self.update_tool_batch_item_state(item_index, new_state);
            }
            self.close_tool_batch_record();
        }
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
            metadata,
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
                metadata: metadata.clone(),
            }),
        ));
    }
    updates
}
