use loopal_turn::{ServerToolPair, TextBlock, ThinkingBlock, ToolCall, ToolResult};

use super::super::message::ContentBlock;

pub(super) fn text_block(block: &TextBlock) -> ContentBlock {
    ContentBlock::Text {
        text: block.text.clone(),
    }
}

pub(super) fn thinking_block(t: &ThinkingBlock) -> ContentBlock {
    ContentBlock::Thinking {
        thinking: t.thinking.clone(),
        signature: t.signature.clone(),
    }
}

pub(super) fn tool_use_block(call: &ToolCall) -> ContentBlock {
    ContentBlock::ToolUse {
        id: call.id.as_str().to_string(),
        name: call.name.clone(),
        input: call.input.clone(),
    }
}

pub(super) fn tool_result_block(tool_use_id: &str, r: &ToolResult) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: tool_use_id.to_string(),
        content: r.content.clone(),
        images: r.images.clone(),
        is_error: r.is_error,
        metadata: None,
    }
}

pub(super) fn server_pair_blocks(pair: &ServerToolPair) -> Vec<ContentBlock> {
    vec![
        ContentBlock::ServerToolUse {
            id: pair.call.id.clone(),
            name: pair.call.name.clone(),
            input: pair.call.input.clone(),
        },
        ContentBlock::ServerToolResult {
            block_type: pair.result.block_type.clone(),
            tool_use_id: pair.call.id.clone(),
            content: pair.result.content.clone(),
        },
    ]
}
