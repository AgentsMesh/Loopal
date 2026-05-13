use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn emit_tool_error(
        &self,
        id: &str,
        name: &str,
        message: &str,
    ) -> loopal_error::Result<()> {
        self.emit(AgentEventPayload::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            result: message.to_string(),
            is_error: true,
            duration_ms: None,
            metadata: None,
        })
        .await
    }

    pub(super) async fn emit_tool_cancelled(
        &self,
        id: &str,
        name: &str,
        message: &str,
    ) -> loopal_error::Result<()> {
        self.emit(AgentEventPayload::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            result: message.to_string(),
            is_error: true,
            duration_ms: None,
            metadata: Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
        })
        .await
    }
}

pub(super) fn error_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: true,
        metadata: None,
    }
}

pub(super) fn cancel_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: true,
        metadata: Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
    }
}
