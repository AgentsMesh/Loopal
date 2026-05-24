use loopal_provider_api::ChatParams;
use loopal_turn::{
    AssistantOutput, CancelCause, CompactionRehydrate, CompactionSummary, OrderedToolBatch,
    ServerToolPair, TextBlock, ThinkingBlock, ToolBatchItem, ToolCall, ToolExecState, ToolResult,
    Turn, TurnStep, TurnTrigger,
};
use serde_json::{Value, json};

use super::AnthropicProvider;
use crate::model_info::get_model_info;

const CONTINUATION_MARKER: &str = "[Continue from where you left off]";

impl AnthropicProvider {
    pub fn build_messages_json_from_turns(&self, params: &ChatParams) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        for turn in &params.turns {
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

// Merge consecutive messages that share the same role into a single message
// with concatenated content. Anthropic's API requires strict user/assistant
// alternation; a cancelled tool batch ending in a User msg followed by a new
// UserInput turn would otherwise produce adjacent Users.
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
    if let Some(msg) = trigger_user(&turn.trigger) {
        out.push(msg);
    }
    for step in &turn.body.steps {
        push_step(out, step);
    }
}

fn trigger_user(trigger: &TurnTrigger) -> Option<Value> {
    // reason: 保持与 turn_projection (provider-api/src/wire/turn_projection.rs)
    // 的前缀语义一致 —— LLM 上下文里 cron/agent/channel/hook 都该见到 originating
    // context 的前缀。
    match trigger {
        TurnTrigger::UserInput { content, .. } => Some(text_user(content)),
        TurnTrigger::Cron { content, .. } => Some(text_user(&format!("[scheduled] {content}"))),
        TurnTrigger::Agent { from, content, .. } => {
            Some(text_user(&format!("[from: {from}] {content}")))
        }
        TurnTrigger::Channel {
            channel,
            from,
            content,
            ..
        } => Some(text_user(&format!("[from: #{channel}/{from}] {content}"))),
        TurnTrigger::GoalContinuation { content, .. } => Some(text_user(content)),
        TurnTrigger::BackgroundHook { content, .. } => Some(text_user(content)),
        TurnTrigger::Resume => None,
    }
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
}

fn build_assistant(response: &AssistantOutput) -> Value {
    let mut content: Vec<Value> = Vec::new();
    if let Some(t) = &response.thinking {
        content.push(thinking_to_json(t));
    }
    for pair in &response.server_blocks {
        content.extend(server_pair_to_json(pair));
    }
    for tb in &response.text_blocks {
        content.push(text_to_json(tb));
    }
    for tc in &response.tool_calls {
        content.push(tool_call_to_json(tc));
    }
    json!({"role": "assistant", "content": content})
}

fn build_user_from_batch(batch: &OrderedToolBatch) -> Value {
    // reason: I4 invariant 通过 batch.items: Vec 顺序锁定 — items 顺序即 ToolCall 顺序，
    // 输出到 wire 的 tool_result 顺序自然匹配 prev assistant 的 tool_use 顺序。
    let content: Vec<Value> = batch.items.iter().map(tool_batch_item_to_json).collect();
    json!({"role": "user", "content": content})
}

fn tool_batch_item_to_json(item: &ToolBatchItem) -> Value {
    let id = item.call.id.as_str();
    match &item.state {
        ToolExecState::Done(r) => tool_result_to_json(id, r),
        ToolExecState::Cancelled(cause) => json!({
            "type": "tool_result",
            "tool_use_id": id,
            "content": cancel_reason(cause),
            "is_error": true,
        }),
        ToolExecState::Pending | ToolExecState::Running => json!({
            "type": "tool_result",
            "tool_use_id": id,
            "content": "Pending — runtime invariant violated",
            "is_error": true,
        }),
    }
}

fn cancel_reason(c: &CancelCause) -> &'static str {
    match c {
        CancelCause::UserInterrupt => "Interrupted by user",
        CancelCause::GovernanceAbort => "Aborted by governance",
        CancelCause::CrashRecovery => "Cancelled (crash recovery)",
        CancelCause::Timeout => "Timed out",
    }
}

fn text_user(text: &str) -> Value {
    json!({"role": "user", "content": [{"type": "text", "text": text}]})
}

fn text_to_json(tb: &TextBlock) -> Value {
    json!({"type": "text", "text": tb.text})
}

fn thinking_to_json(t: &ThinkingBlock) -> Value {
    json!({
        "type": "thinking",
        "thinking": t.thinking,
        "signature": t.signature.as_deref().unwrap_or(""),
    })
}

fn tool_call_to_json(c: &ToolCall) -> Value {
    json!({
        "type": "tool_use",
        "id": c.id.as_str(),
        "name": c.name,
        "input": c.input,
    })
}

fn tool_result_to_json(tool_use_id: &str, r: &ToolResult) -> Value {
    // reason: when images attach, content must become an array of blocks
    // [text?, image*] per Anthropic API. SessionResource is unexpected here
    // (hydrate should have converted it to Inline already).
    let content_value = if r.images.is_empty() {
        json!(r.content)
    } else {
        let mut blocks: Vec<Value> = Vec::new();
        if !r.content.is_empty() {
            blocks.push(json!({"type": "text", "text": r.content}));
        }
        for img in &r.images {
            let Some((media_type, data)) = img.as_inline() else {
                continue;
            };
            blocks.push(json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": media_type,
                    "data": data,
                }
            }));
        }
        json!(blocks)
    };
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use_id,
        "content": content_value,
        "is_error": r.is_error,
    })
}

fn server_pair_to_json(p: &ServerToolPair) -> Vec<Value> {
    vec![
        json!({
            "type": "server_tool_use",
            "id": p.call.id,
            "name": p.call.name,
            "input": p.call.input,
        }),
        json!({
            "type": p.result.block_type,
            "tool_use_id": p.call.id,
            "content": p.result.content,
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(text: &str) -> Value {
        json!({"role": "user", "content": [{"type": "text", "text": text}]})
    }
    fn assistant(text: &str) -> Value {
        json!({"role": "assistant", "content": [{"type": "text", "text": text}]})
    }

    #[test]
    fn merge_collapses_adjacent_user_msgs() {
        let mut v = vec![user("a"), user("b"), assistant("c"), user("d")];
        merge_adjacent_same_role(&mut v);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0]["role"], "user");
        assert_eq!(v[0]["content"].as_array().unwrap().len(), 2);
        assert_eq!(v[1]["role"], "assistant");
        assert_eq!(v[2]["role"], "user");
    }

    #[test]
    fn merge_collapses_adjacent_assistant_msgs() {
        let mut v = vec![user("a"), assistant("b"), assistant("c"), user("d")];
        merge_adjacent_same_role(&mut v);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1]["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn merge_noop_on_alternating() {
        let mut v = vec![user("a"), assistant("b"), user("c"), assistant("d")];
        let before = v.clone();
        merge_adjacent_same_role(&mut v);
        assert_eq!(v, before);
    }

    #[test]
    fn merge_handles_empty_and_single() {
        let mut empty: Vec<Value> = vec![];
        merge_adjacent_same_role(&mut empty);
        assert!(empty.is_empty());

        let mut one = vec![user("a")];
        merge_adjacent_same_role(&mut one);
        assert_eq!(one.len(), 1);
    }
}
