use loopal_turn::{ServerBlock, ToolExecState, Turn, TurnStep};

use crate::budget::ContextBudget;
use crate::compact_config::{
    LAYER1_TRIGGER_PERCENT, LAYER1_TRUNCATE_MAX_BYTES, LAYER1_TRUNCATE_MAX_LINES,
};
use crate::ingestion::condense_server_pairs_into_text;
use crate::token_counter::estimate_tokens;

const CAP_MAX_LINES: usize = 500;
const CAP_MAX_BYTES: usize = 20_000;

pub fn degrade_turns_for_wire(turns: &mut [Turn], budget: &ContextBudget) {
    if turns.is_empty() {
        return;
    }
    let max_result_tokens = budget.message_budget / 8;
    let last_idx = turns.len() - 1;

    for (i, turn) in turns.iter_mut().enumerate() {
        let is_current = i == last_idx;
        for step in &mut turn.body.steps {
            match step {
                TurnStep::LlmCall { response, .. } => {
                    if !is_current {
                        // 旧 turn：web_search_call 被 condense 成 text 后不复存在，
                        // 其 reasoning 锚点(及 Anthropic thinking)无需保留 → 全清。
                        condense_server_pairs_into_text(response);
                    }
                }
                TurnStep::ToolBatch(batch) => {
                    for item in &mut batch.items {
                        let ToolExecState::Done(r) = &mut item.state else {
                            continue;
                        };
                        if r.is_error {
                            continue;
                        }
                        if !is_current && !r.images.is_empty() {
                            r.images.clear();
                        }
                        if exceeds_content_limits(&r.content, CAP_MAX_LINES, CAP_MAX_BYTES) {
                            truncate_content_in_place(&mut r.content, CAP_MAX_LINES, CAP_MAX_BYTES);
                        }
                        let tokens = estimate_tokens(&r.content);
                        if tokens > max_result_tokens {
                            truncate_content_in_place(&mut r.content, CAP_MAX_LINES, CAP_MAX_BYTES);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let total = estimate_turns_tokens(turns);
    if total > budget.message_budget * LAYER1_TRIGGER_PERCENT / 100 {
        truncate_older_results(turns, budget);
    }
}

fn truncate_older_results(turns: &mut [Turn], budget: &ContextBudget) {
    if turns.len() <= 1 {
        return;
    }
    let threshold = budget.message_budget / 8;
    let last_idx = turns.len() - 1;
    for (i, turn) in turns.iter_mut().enumerate() {
        if i >= last_idx {
            break;
        }
        for step in &mut turn.body.steps {
            let TurnStep::ToolBatch(batch) = step else {
                continue;
            };
            for item in &mut batch.items {
                let ToolExecState::Done(r) = &mut item.state else {
                    continue;
                };
                if r.is_error {
                    continue;
                }
                let tokens = estimate_tokens(&r.content);
                if tokens > threshold {
                    truncate_content_in_place(
                        &mut r.content,
                        LAYER1_TRUNCATE_MAX_LINES,
                        LAYER1_TRUNCATE_MAX_BYTES,
                    );
                }
            }
        }
    }
}

fn exceeds_content_limits(content: &str, max_lines: usize, max_bytes: usize) -> bool {
    content.len() > max_bytes || content.lines().take(max_lines + 1).count() > max_lines
}

fn truncate_content_in_place(content: &mut String, max_lines: usize, max_bytes: usize) {
    if !exceeds_content_limits(content, max_lines, max_bytes) {
        return;
    }
    let original_bytes = content.len();
    let original_lines = content.lines().count();
    let truncated = loopal_tool_api::truncate_output(content, max_lines, max_bytes);
    let kept_bytes = truncated.len().min(original_bytes);
    *content = format!(
        "{truncated}\n[Truncated: kept {kept_bytes}/{original_bytes} bytes, \
         approx {max_lines}/{original_lines} lines]"
    );
}

pub fn estimate_turns_tokens(turns: &[Turn]) -> u32 {
    let mut total = 0u32;
    for turn in turns {
        for step in &turn.body.steps {
            match step {
                TurnStep::LlmCall { response, .. } => {
                    for t in &response.text_blocks {
                        total = total.saturating_add(estimate_tokens(&t.text));
                    }
                    for block in &response.server_blocks {
                        if let ServerBlock::Reasoning(t) = block {
                            total = total.saturating_add(estimate_tokens(&t.thinking));
                        }
                    }
                }
                TurnStep::ToolBatch(batch) => {
                    for item in &batch.items {
                        if let ToolExecState::Done(r) = &item.state {
                            total = total.saturating_add(estimate_tokens(&r.content));
                        }
                    }
                }
                TurnStep::CompactionSummary(s) => {
                    total = total.saturating_add(estimate_tokens(&s.summary_text));
                }
                TurnStep::Injection { text, .. } => {
                    total = total.saturating_add(estimate_tokens(text));
                }
                _ => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests;
