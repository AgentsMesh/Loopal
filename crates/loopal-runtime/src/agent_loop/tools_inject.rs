use loopal_error::Result;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use tracing::info;

use super::runner::AgentLoopRunner;
use super::tool_result_sink::PendingToolResult;

impl AgentLoopRunner {
    pub(super) async fn pending_tool_result(
        &self,
        id: &str,
        name: &str,
        content: impl Into<String>,
        is_error: bool,
        metadata: Option<ToolResultMetadata>,
    ) -> Result<PendingToolResult> {
        Ok(PendingToolResult::new(
            id, name, content, is_error, metadata,
        ))
    }

    pub(super) fn all_interrupted(
        &self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Vec<(usize, PendingToolResult)> {
        info!("cancelled, skipping tool execution");
        let metadata = ToolResultMetadata::cancelled(CancelCause::UserInterrupt);
        tool_uses
            .iter()
            .enumerate()
            .map(|(index, (id, name, _))| {
                (
                    index,
                    PendingToolResult::new(
                        id,
                        name,
                        "Interrupted by user",
                        true,
                        Some(metadata.clone()),
                    ),
                )
            })
            .collect()
    }
}
