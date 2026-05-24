use loopal_provider_api::ChatParams;
use loopal_turn::{
    AssistantOutput, CancelCause, CompactionRehydrate, CompactionSummary, OrderedToolBatch,
    ServerToolPair, TextBlock, ThinkingBlock, ToolBatchItem, ToolCall, ToolExecState, ToolResult,
    Turn, TurnStep, TurnTrigger,
};
use serde_json::{Value, json};

use super::AnthropicProvider;

impl AnthropicProvider {
    pub fn build_messages_json_from_turns(&self, params: &ChatParams) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        for turn in &params.turns {
            push_turn(&mut out, turn);
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

fn push_turn(out: &mut Vec<Value>, turn: &Turn) {
    if let Some(msg) = trigger_user(&turn.trigger) {
        out.push(msg);
    }
    for step in &turn.body.steps {
        push_step(out, step);
    }
}

fn trigger_user(trigger: &TurnTrigger) -> Option<Value> {
    match trigger {
        TurnTrigger::UserInput { content, .. } => Some(text_user(content)),
        TurnTrigger::Cron { prompt, .. } => Some(text_user(prompt)),
        TurnTrigger::GoalContinuation { .. }
        | TurnTrigger::BackgroundHook { .. }
        | TurnTrigger::Resume => None,
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
        TurnStep::Injection(inj) => out.push(text_user(&inj.text)),
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
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use_id,
        "content": r.content,
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
