use loopal_tool_api::COMPLETION_PREFIX;

use crate::state::SessionState;
use crate::truncate::truncate_result_for_storage;
use crate::types::DisplayMessage;

/// Handle ToolResult: update status, and promote AttemptCompletion to assistant message.
pub(crate) fn handle_tool_result(
    state: &mut SessionState,
    name: String,
    result: String,
    is_error: bool,
) {
    let status = if is_error { "error" } else { "success" };
    let is_completion = name == "AttemptCompletion" && !is_error;
    'outer: for msg in state.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            if tc.name == name && tc.status == "pending" {
                tc.status = status.to_string();
                if !is_completion {
                    tc.result = Some(truncate_result_for_storage(&result));
                }
                break 'outer;
            }
        }
    }
    // Promote AttemptCompletion to assistant message (prefix from AttemptCompletionTool)
    if is_completion {
        let content = result.strip_prefix(COMPLETION_PREFIX).unwrap_or(&result);
        state.messages.push(DisplayMessage {
            role: "assistant".into(),
            content: content.to_string(),
            tool_calls: Vec::new(),
            image_count: 0,
        });
    }
}
