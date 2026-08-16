use loopal_error::Result;
use loopal_provider_api::ContentBlock;
use loopal_turn::{ToolExecState, ToolResult};

use super::runner::AgentLoopRunner;
use super::tool_result_sink::PendingToolResult;

impl AgentLoopRunner {
    pub(super) async fn finalize_tool_results(
        &mut self,
        mut indexed_results: Vec<(usize, PendingToolResult)>,
    ) -> Result<u32> {
        if indexed_results.is_empty() {
            return Ok(0);
        }
        indexed_results.sort_by_key(|(index, _)| *index);
        let mut finalized = Vec::with_capacity(indexed_results.len());
        for (index, pending) in indexed_results {
            finalized.push((index, pending.finalize(self).await?));
        }
        let emitter = self.params.deps.frontend.event_emitter();
        for (_, result) in &finalized {
            loopal_protocol::event_id::scope_turn(
                result.context.turn_id,
                loopal_protocol::event_id::scope_correlation(
                    result.context.correlation_id,
                    emitter.emit_best_effort(
                        result.event.clone(),
                        "agent_loop::tools_finalize::tool_result",
                    ),
                ),
            )
            .await;
        }
        let blocks = finalized
            .into_iter()
            .map(|(index, result)| (index, result.block))
            .collect::<Vec<_>>();
        let errors = blocks
            .iter()
            .filter(|(_, block)| matches!(block, ContentBlock::ToolResult { is_error: true, .. }))
            .count() as u32;
        let item_updates = collect_item_updates(&blocks, self.current_tool_batch_item_ids());
        if self.turns.current_tool_batch_step().is_some() {
            for (item_index, new_state) in item_updates {
                self.update_tool_batch_item_state(item_index, new_state);
            }
            self.close_tool_batch_record();
        }
        Ok(errors)
    }

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
            .map(|item| item.call.id.as_str().to_string())
            .collect()
    }
}

fn collect_item_updates(
    blocks: &[(usize, ContentBlock)],
    batch_ids: Vec<String>,
) -> Vec<(u32, ToolExecState)> {
    let mut updates = Vec::with_capacity(batch_ids.len());
    for (_, block) in blocks {
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

#[cfg(test)]
#[path = "tools_finalize_tests.rs"]
mod tests;
