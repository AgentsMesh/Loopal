use loopal_provider_api::MessageOrigin;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};

use super::super::tools_inject::tool_result_block;

pub fn synthesize_aborted_tool_results(
    tool_uses: &[(String, String, serde_json::Value)],
    reason: &str,
) -> Option<Message> {
    if tool_uses.is_empty() {
        return None;
    }
    let metadata = ToolResultMetadata::Cancelled {
        cause: CancelCause::GovernanceAbort,
    };
    let blocks: Vec<ContentBlock> = tool_uses
        .iter()
        .map(|(id, name, _)| {
            tool_result_block(
                id,
                &format!("[aborted: {reason}] tool={name}"),
                true,
                Some(metadata.clone()),
            )
        })
        .collect();
    Some(Message {
        id: None,
        role: MessageRole::User,
        content: blocks,
        origin: Some(MessageOrigin::GovernanceCompensation),
        ephemeral_in_history: false,
    })
}
