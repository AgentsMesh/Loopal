use loopal_turn::{
    AssistantOutput, CancelCause, OrderedToolBatch, ServerBlock, ServerToolPair, TextBlock,
    ThinkingBlock, ToolBatchItem, ToolCall, ToolExecState, ToolResult,
};
use serde_json::{Value, json};

pub(super) fn build_assistant(response: &AssistantOutput) -> Value {
    let mut content: Vec<Value> = Vec::new();
    // reason: server_blocks 在最前 → thinking 必须是 assistant content 首块(Anthropic
    // 硬约束)且紧贴其 server_tool_use。
    for block in &response.server_blocks {
        match block {
            ServerBlock::Reasoning(t) => content.extend(thinking_to_json(t)),
            ServerBlock::ToolPair(p) => content.extend(server_pair_to_json(p)),
        }
    }
    for tb in &response.text_blocks {
        content.push(text_to_json(tb));
    }
    for tc in &response.tool_calls {
        content.push(tool_call_to_json(tc));
    }
    json!({"role": "assistant", "content": content})
}

pub(super) fn build_user_from_batch(batch: &OrderedToolBatch) -> Value {
    let content: Vec<Value> = batch.items.iter().map(tool_batch_item_to_json).collect();
    json!({"role": "user", "content": content})
}

pub(super) fn text_user(text: &str) -> Value {
    json!({"role": "user", "content": [{"type": "text", "text": text}]})
}

pub(super) fn user_input<'a>(
    text: &str,
    images: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Value {
    let mut content = Vec::new();
    if !text.is_empty() {
        content.push(json!({"type": "text", "text": text}));
    }
    for (media_type, data) in images {
        content.push(json!({
            "type": "image",
            "source": {"type": "base64", "media_type": media_type, "data": data},
        }));
    }
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
        CancelCause::ParentTurnAborted => "Cancelled (superseded by new input)",
    }
}

fn text_to_json(tb: &TextBlock) -> Value {
    json!({"type": "text", "text": tb.text})
}

// reason: Anthropic 对 thinking 块密码学校验 signature，空/缺签名会 400。无签名的
// Reasoning 直接跳过(纵深防御),正常 extended-thinking 流必带签名。
fn thinking_to_json(t: &ThinkingBlock) -> Option<Value> {
    let signature = t.signature.as_deref().filter(|s| !s.is_empty())?;
    Some(json!({
        "type": "thinking",
        "thinking": t.thinking,
        "signature": signature,
    }))
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
