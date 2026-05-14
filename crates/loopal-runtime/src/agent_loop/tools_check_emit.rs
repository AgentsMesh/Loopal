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
        self.emit_in_turn(AgentEventPayload::ToolResult {
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
        self.emit_in_turn(AgentEventPayload::ToolResult {
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
