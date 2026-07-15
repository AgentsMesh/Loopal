mod to_json;

use loopal_provider_api::ChatParams;
use loopal_turn::{CompactionRehydrate, CompactionSummary, Turn, TurnStep, TurnTrigger};
use serde_json::{Value, json};

use self::to_json::{build_assistant, build_user_from_batch, text_user, user_input};
use super::AnthropicProvider;
use crate::model_info::get_model_info;

const CONTINUATION_MARKER: &str = "[Continue from where you left off]";

impl AnthropicProvider {
    pub fn build_messages_json_from_turns(&self, params: &ChatParams) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        let drop_before = find_compaction_drop_index(&params.turns);
        for turn in params.turns.iter().skip(drop_before) {
            push_turn(&mut out, turn);
        }
        merge_adjacent_same_role(&mut out);
        let supports_prefill = get_model_info(&params.model)
            .map(|m| m.supports_prefill)
            .unwrap_or(true);
        let needs_user_tail = !supports_prefill || params.continuation_intent.is_some();
        if needs_user_tail && !out.last().is_some_and(|m| m["role"] == "user") {
            out.push(json!({
                "role": "user",
                "content": [{"type": "text", "text": CONTINUATION_MARKER}],
            }));
        }
        if let Some(last_user) = out.iter_mut().rev().find(|m| m["role"] == "user")
            && let Some(arr) = last_user["content"].as_array_mut()
            && let Some(last_block) = arr.last_mut()
        {
            last_block["cache_control"] = json!({"type": "ephemeral"});
        }
        out
    }
}

/// Turn-level compaction boundary: latest turn carrying a CompactionSummary.
/// All turns before that index are dropped from wire — represented by the
/// summary. Boundary turn kept intact so LLM has User-tailed conversation.
fn find_compaction_drop_index(turns: &[Turn]) -> usize {
    for (i, turn) in turns.iter().enumerate().rev() {
        if turn
            .body
            .steps
            .iter()
            .any(|s| matches!(s, TurnStep::CompactionSummary(_)))
        {
            return i;
        }
    }
    0
}

fn merge_adjacent_same_role(out: &mut Vec<Value>) {
    let mut write = 0usize;
    for read in 0..out.len() {
        if write > 0 && out[write - 1]["role"] == out[read]["role"] {
            let extra = std::mem::take(&mut out[read]["content"]);
            if let (Some(dst), Some(src)) =
                (out[write - 1]["content"].as_array_mut(), extra.as_array())
            {
                dst.extend(src.iter().cloned());
            }
            continue;
        }
        if write != read {
            out.swap(write, read);
        }
        write += 1;
    }
    out.truncate(write);
}

fn push_turn(out: &mut Vec<Value>, turn: &Turn) {
    // CompactionSummary must project BEFORE the turn's trigger; otherwise an
    // in-progress UserInput turn with appended summary emits [user X,
    // summary, ack] (summary AFTER user query). Multiple summaries collapse
    // ordering — debug_assert pins the invariant and the fold-to-latest
    // recovery keeps wire-shape correct in release.
    let summary_steps: Vec<&TurnStep> = turn
        .body
        .steps
        .iter()
        .filter(|s| matches!(s, TurnStep::CompactionSummary(_)))
        .collect();
    debug_assert!(
        summary_steps.len() <= 1,
        "turn has multiple CompactionSummary steps; hoist loses ordering"
    );
    if let Some(latest) = summary_steps.last() {
        push_step(out, latest);
    }
    if let Some(msg) = trigger_user(&turn.trigger) {
        out.push(msg);
    }
    for step in &turn.body.steps {
        if !matches!(step, TurnStep::CompactionSummary(_)) {
            push_step(out, step);
        }
    }
}

fn trigger_user(trigger: &TurnTrigger) -> Option<Value> {
    if let TurnTrigger::UserInput {
        content, images, ..
    } = trigger
    {
        return Some(user_input(
            content,
            images.iter().filter_map(|image| image.as_inline()),
        ));
    }
    loopal_provider_api::trigger_llm_text(trigger).map(|text| text_user(&text))
}

fn push_step(out: &mut Vec<Value>, step: &TurnStep) {
    match step {
        TurnStep::LlmCall { response, .. } => out.push(build_assistant(response)),
        TurnStep::ToolBatch(batch) if !batch.items.is_empty() => {
            out.push(build_user_from_batch(batch));
        }
        TurnStep::ToolBatch(_) => {}
        TurnStep::CompactionSummary(s) => push_compaction_summary(out, s),
        TurnStep::CompactionRehydrate(r) => push_compaction_rehydrate(out, r),
        TurnStep::Injection { text, .. } => out.push(text_user(text)),
    }
}

fn push_compaction_summary(out: &mut Vec<Value>, s: &CompactionSummary) {
    if !s.summary_text.is_empty() {
        out.push(text_user(&s.summary_text));
    }
    if !s.ack_text.is_empty() {
        out.push(json!({"role": "assistant", "content": [{"type":"text","text": s.ack_text}]}));
    }
}

fn push_compaction_rehydrate(out: &mut Vec<Value>, r: &CompactionRehydrate) {
    for f in &r.files {
        out.push(json!({
            "role": "assistant",
            "content": [{
                "type": "tool_use",
                "id": f.tool_call_id.as_str(),
                "name": "Read",
                "input": {"file_path": f.path}
            }]
        }));
        out.push(json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": f.tool_call_id.as_str(),
                "content": f.content,
                "is_error": false
            }]
        }));
    }
    if let Some(note) = r.partial_note.as_ref() {
        out.push(json!({
            "role": "user",
            "content": [{"type": "text", "text": note}],
        }));
    }
}

#[cfg(test)]
mod tests_boundary;
#[cfg(test)]
mod tests_merge;
#[cfg(test)]
mod tests_reasoning;
