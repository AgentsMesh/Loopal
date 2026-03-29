//! Translate tool-related agent events to ACP SessionUpdate.

use agent_client_protocol_schema::{
    SessionUpdate, ToolCall, ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
};

use super::tool_kind::map_tool_kind;

/// `ToolCall { id, name, .. }` → `SessionUpdate::ToolCall`
pub fn translate_tool_call(id: &str, name: &str) -> SessionUpdate {
    SessionUpdate::ToolCall(
        ToolCall::new(ToolCallId::new(id), name.to_string())
            .kind(map_tool_kind(name))
            .status(ToolCallStatus::Pending),
    )
}

/// `ToolResult { id, result, is_error, .. }` → `SessionUpdate::ToolCallUpdate`
pub fn translate_tool_result(id: &str, result: &str, is_error: bool) -> SessionUpdate {
    let status = if is_error {
        ToolCallStatus::Failed
    } else {
        ToolCallStatus::Completed
    };
    let fields = ToolCallUpdateFields::new()
        .status(status)
        .raw_output(serde_json::Value::String(result.to_string()));
    SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(ToolCallId::new(id), fields))
}

/// `ToolProgress { id, output_tail, .. }` → `SessionUpdate::ToolCallUpdate { InProgress }`
pub fn translate_tool_progress(id: &str, output_tail: &str) -> SessionUpdate {
    let fields = ToolCallUpdateFields::new()
        .status(ToolCallStatus::InProgress)
        .raw_output(serde_json::Value::String(output_tail.to_string()));
    SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(ToolCallId::new(id), fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_has_pending_status() {
        let update = translate_tool_call("tc-1", "Read");
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "tool_call");
        assert_eq!(val["toolCallId"], "tc-1");
        assert_eq!(val["title"], "Read");
        assert_eq!(val["status"], "pending");
        assert_eq!(val["kind"], "read");
    }

    #[test]
    fn tool_result_success_is_completed() {
        let update = translate_tool_result("tc-1", "file contents", false);
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "tool_call_update");
        assert_eq!(val["toolCallId"], "tc-1");
        assert_eq!(val["status"], "completed");
        assert_eq!(val["rawOutput"], "file contents");
    }

    #[test]
    fn tool_result_error_is_failed() {
        let update = translate_tool_result("tc-1", "not found", true);
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["status"], "failed");
    }

    #[test]
    fn tool_progress_is_in_progress() {
        let update = translate_tool_progress("tc-1", "running...");
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "tool_call_update");
        assert_eq!(val["status"], "in_progress");
        assert_eq!(val["rawOutput"], "running...");
    }
}
