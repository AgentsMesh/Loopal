//! Projection helpers from persisted history (`ProjectedMessage`) to
//! display state (`SessionMessage`). The TUI calls
//! `App::load_display_history` / `App::load_sub_agent_history`, both of
//! which delegate here for the message-shape conversion.

use loopal_protocol::ProjectedMessage;
use loopal_view_state::{SessionMessage, SessionToolCall, ToolCallStatus};

pub fn into_session_message(p: ProjectedMessage) -> SessionMessage {
    SessionMessage {
        role: p.role,
        content: p.content,
        tool_calls: p
            .tool_calls
            .into_iter()
            .map(|tc| SessionToolCall {
                id: tc.id,
                name: tc.name.clone(),
                status: if tc.is_error {
                    ToolCallStatus::Error
                } else if tc.result.is_some() {
                    ToolCallStatus::Success
                } else {
                    ToolCallStatus::Pending
                },
                summary: tc.summary,
                result: tc.result,
                tool_input: tc.input,
                batch_id: None,
                started_at: None,
                duration_ms: None,
                progress_tail: None,
                metadata: tc.metadata,
            })
            .collect(),
        image_count: p.image_count,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    }
}
