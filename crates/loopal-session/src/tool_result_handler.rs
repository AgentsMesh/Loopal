use loopal_tool_api::COMPLETION_PREFIX;

use crate::state::SessionState;
use crate::truncate::truncate_result_for_storage;
use crate::types::{DisplayMessage, ToolCallStatus};

/// Handle ToolResult: update status, duration, and promote AttemptCompletion.
pub(crate) fn handle_tool_result(
    state: &mut SessionState,
    id: String,
    name: String,
    result: String,
    is_error: bool,
    duration_ms: Option<u64>,
) {
    let status = if is_error {
        ToolCallStatus::Error
    } else {
        ToolCallStatus::Success
    };
    let is_completion = name == "AttemptCompletion" && !is_error;
    'outer: for msg in state.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            let matches = if !id.is_empty() {
                tc.id == id
            } else {
                tc.name == name && tc.status == ToolCallStatus::Pending
            };
            if matches {
                tc.status = status;
                tc.duration_ms = duration_ms;
                tc.progress_tail = None;
                if !is_completion {
                    tc.result = Some(truncate_result_for_storage(&result));
                }
                break 'outer;
            }
        }
    }
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

/// Mark pending tools as belonging to a parallel batch.
pub(crate) fn handle_tool_batch_start(state: &mut SessionState, tool_ids: Vec<String>) {
    let batch_id = format!("batch-{}", state.turn_count);
    for msg in state.messages.iter_mut().rev() {
        if msg.role != "assistant" || msg.tool_calls.is_empty() {
            continue;
        }
        let mut found = false;
        for tc in msg.tool_calls.iter_mut() {
            if tc.status == ToolCallStatus::Pending && tool_ids.contains(&tc.id) {
                tc.batch_id = Some(batch_id.clone());
                found = true;
            }
        }
        if found {
            break;
        }
    }
}

/// Update a running tool's progress tail (for long-running Bash commands).
pub(crate) fn handle_tool_progress(state: &mut SessionState, id: String, output_tail: String) {
    for msg in state.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            if tc.id == id {
                if tc.status.is_done() {
                    return;
                }
                tc.status = ToolCallStatus::Running;
                tc.progress_tail = Some(output_tail);
                return;
            }
        }
    }
}
