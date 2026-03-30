use std::time::Instant;

use crate::helpers::flush_streaming;
use crate::state::SessionState;
use crate::truncate::{truncate_json, truncate_result_for_storage};
use crate::types::{DisplayMessage, DisplayToolCall, ToolCallStatus};

/// Handle ToolCall: create a pending DisplayToolCall and attach to the last assistant message.
pub(crate) fn handle_tool_call(
    state: &mut SessionState,
    id: String,
    name: String,
    input: serde_json::Value,
) {
    flush_streaming(state);
    let tc = DisplayToolCall {
        id: id.clone(),
        name: name.clone(),
        status: ToolCallStatus::Pending,
        summary: if name == "AttemptCompletion" {
            name.clone()
        } else {
            format!("{}({})", name, truncate_json(&input, 60))
        },
        result: None,
        tool_input: Some(input),
        batch_id: None,
        started_at: Some(Instant::now()),
        duration_ms: None,
        progress_tail: None,
        metadata: None,
    };
    if let Some(last) = state.messages.last_mut()
        && last.role == "assistant"
    {
        last.tool_calls.push(tc);
        return;
    }
    state.messages.push(DisplayMessage {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: vec![tc],
        image_count: 0,
        skill_info: None,
    });
}

/// Parameters for a tool result update.
pub(crate) struct ToolResultParams {
    pub id: String,
    pub name: String,
    pub result: String,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
    pub is_completion: bool,
    pub metadata: Option<serde_json::Value>,
}

/// Handle ToolResult: update status, duration, and promote AttemptCompletion.
pub(crate) fn handle_tool_result(state: &mut SessionState, p: ToolResultParams) {
    let status = if p.is_error {
        ToolCallStatus::Error
    } else {
        ToolCallStatus::Success
    };
    'outer: for msg in state.messages.iter_mut().rev() {
        for tc in msg.tool_calls.iter_mut().rev() {
            let matches = if !p.id.is_empty() {
                tc.id == p.id
            } else {
                tc.name == p.name && tc.status == ToolCallStatus::Pending
            };
            if matches {
                tc.status = status;
                tc.duration_ms = p.duration_ms;
                tc.progress_tail = None;
                tc.metadata = p.metadata.clone();
                if !p.is_completion {
                    tc.result = Some(truncate_result_for_storage(&p.result));
                }
                break 'outer;
            }
        }
    }
    if p.is_completion {
        state.messages.push(DisplayMessage {
            role: "assistant".into(),
            content: p.result,
            tool_calls: Vec::new(),
            image_count: 0,
            skill_info: None,
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
