use loopal_turn::{ToolExecState, TurnStep};

use crate::turn_store::TurnStore;

// Resume-mid-batch path: fold_events surfaces an InProgress turn whose last
// step is a ToolBatch with Pending/Running items. Without pre-populating
// current_tool_batch_step, subsequent update_tool_state calls hit
// NoToolBatchOpen and silently drop StepUpdated events.
//
// Only inspect the LAST ToolBatch — an earlier Pending batch followed by a
// closed batch is malformed; routing writes to the older batch would mutate
// items in a batch the rest of the runtime treats as closed.
pub(super) fn derive_current_tool_batch_step(store: &TurnStore) -> Option<u32> {
    let turn = store.current_turn()?;
    let (last_batch_idx, last_batch) =
        turn.body
            .steps
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, s)| match s {
                TurnStep::ToolBatch(b) => Some((i, b)),
                _ => None,
            })?;
    let has_pending = last_batch
        .items
        .iter()
        .any(|it| matches!(it.state, ToolExecState::Pending | ToolExecState::Running));
    has_pending.then_some(last_batch_idx as u32)
}

#[cfg(test)]
mod tests;
