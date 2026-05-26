use loopal_turn::{
    CancelCause, CancelledCause, OrderedToolBatch, ToolBatchItem, ToolExecState, Turn, TurnOutcome,
    TurnStep,
};

pub fn finalize_incomplete_turns(turns: &mut [Turn]) {
    for turn in turns.iter_mut() {
        if matches!(turn.outcome, TurnOutcome::InProgress) {
            turn.outcome = TurnOutcome::Cancelled {
                cause: CancelledCause::CrashRecovery,
            };
            for step in turn.body.steps.iter_mut() {
                if let TurnStep::ToolBatch(batch) = step {
                    for item in batch.items.iter_mut() {
                        if matches!(item.state, ToolExecState::Pending | ToolExecState::Running) {
                            item.state = ToolExecState::Cancelled(CancelCause::CrashRecovery);
                        }
                    }
                }
            }
        }
        synthesize_missing_tool_batches(turn);
    }
}

pub fn synthesize_missing_tool_batches(turn: &mut Turn) {
    let original_steps = turn.body.steps.clone();
    let mut rebuilt: Vec<TurnStep> = Vec::with_capacity(original_steps.len());
    let mut consumed: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for (idx, step) in original_steps.iter().enumerate() {
        if consumed.contains(&idx) {
            continue;
        }
        if let TurnStep::LlmCall { response, .. } = step
            && !response.tool_calls.is_empty()
        {
            let calls = response.tool_calls.clone();
            let next_llmcall_with_tools = original_steps
                .iter()
                .enumerate()
                .skip(idx + 1)
                .find(|(_, s)| {
                    matches!(s, TurnStep::LlmCall { response, .. } if !response.tool_calls.is_empty())
                })
                .map(|(i, _)| i)
                .unwrap_or(original_steps.len());
            let batch_pos = original_steps
                .iter()
                .enumerate()
                .skip(idx + 1)
                .take_while(|(i, _)| *i < next_llmcall_with_tools)
                .find(|(i, s)| !consumed.contains(i) && matches!(s, TurnStep::ToolBatch(_)))
                .map(|(i, _)| i);
            rebuilt.push(step.clone());
            match batch_pos {
                Some(pos) => {
                    let mut batch = if let TurnStep::ToolBatch(b) = &original_steps[pos] {
                        b.clone()
                    } else {
                        unreachable!()
                    };
                    let already: std::collections::HashSet<&str> =
                        batch.items.iter().map(|i| i.call.id.as_str()).collect();
                    let missing: Vec<_> = calls
                        .iter()
                        .filter(|c| !already.contains(c.id.as_str()))
                        .cloned()
                        .collect();
                    for call in missing {
                        batch.items.push(ToolBatchItem {
                            call,
                            state: ToolExecState::Cancelled(CancelCause::CrashRecovery),
                        });
                    }
                    for (inter_idx, inter_step) in
                        original_steps.iter().enumerate().take(pos).skip(idx + 1)
                    {
                        rebuilt.push(inter_step.clone());
                        consumed.insert(inter_idx);
                    }
                    rebuilt.push(TurnStep::ToolBatch(batch));
                    consumed.insert(pos);
                }
                None => {
                    let already_paired: std::collections::HashSet<&str> = original_steps
                        .iter()
                        .skip(idx + 1)
                        .filter_map(|s| match s {
                            TurnStep::ToolBatch(b) => {
                                Some(b.items.iter().map(|i| i.call.id.as_str()))
                            }
                            _ => None,
                        })
                        .flatten()
                        .collect();
                    let items: Vec<_> = calls
                        .into_iter()
                        .filter(|c| !already_paired.contains(c.id.as_str()))
                        .map(|call| ToolBatchItem {
                            call,
                            state: ToolExecState::Cancelled(CancelCause::CrashRecovery),
                        })
                        .collect();
                    if !items.is_empty() {
                        rebuilt.push(TurnStep::ToolBatch(OrderedToolBatch { items }));
                    }
                }
            }
        } else {
            rebuilt.push(step.clone());
        }
    }
    turn.body.steps = rebuilt;
}
