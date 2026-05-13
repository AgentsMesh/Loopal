use agent_client_protocol_schema::{
    SessionUpdate, ToolCall, ToolCallId, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
};
use loopal_tool_invocation::ToolResultMetadata;

use super::tool_kind::map_tool_kind;

pub fn translate_tool_call(id: &str, name: &str) -> SessionUpdate {
    SessionUpdate::ToolCall(
        ToolCall::new(ToolCallId::new(id), name.to_string())
            .kind(map_tool_kind(name))
            .status(ToolCallStatus::Pending),
    )
}

pub fn translate_tool_result(
    id: &str,
    result: &str,
    is_error: bool,
    metadata: Option<&ToolResultMetadata>,
) -> SessionUpdate {
    let (status, output) = match metadata {
        Some(ToolResultMetadata::Stale { reason }) => (
            ToolCallStatus::Failed,
            serde_json::Value::String(format!("Stale ({reason}): {result}")),
        ),
        Some(ToolResultMetadata::Cancelled { cause }) => (
            ToolCallStatus::Failed,
            serde_json::Value::String(format!("Cancelled ({cause}): {result}")),
        ),
        _ => {
            let status = if is_error {
                ToolCallStatus::Failed
            } else {
                ToolCallStatus::Completed
            };
            (status, serde_json::Value::String(result.to_string()))
        }
    };
    let fields = ToolCallUpdateFields::new()
        .status(status)
        .raw_output(output);
    SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(ToolCallId::new(id), fields))
}

pub fn translate_tool_progress(id: &str, output_tail: &str) -> SessionUpdate {
    let fields = ToolCallUpdateFields::new()
        .status(ToolCallStatus::InProgress)
        .raw_output(serde_json::Value::String(output_tail.to_string()));
    SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(ToolCallId::new(id), fields))
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_tool_invocation::{CancelCause, StaleReason};

    #[test]
    fn tool_call_has_pending_status() {
        let update = translate_tool_call("tc-1", "Read");
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "tool_call");
        assert_eq!(val["toolCallId"], "tc-1");
        assert_eq!(val["title"], "Read");
        // ACP schema serializes `status: Pending` as absent (default + skip_serializing_if).
        assert!(val.get("status").is_none() || val["status"].is_null());
        assert_eq!(val["kind"], "read");
    }

    #[test]
    fn tool_result_success_is_completed() {
        let update = translate_tool_result("tc-1", "file contents", false, None);
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["sessionUpdate"], "tool_call_update");
        assert_eq!(val["toolCallId"], "tc-1");
        assert_eq!(val["status"], "completed");
        assert_eq!(val["rawOutput"], "file contents");
    }

    #[test]
    fn tool_result_error_is_failed() {
        let update = translate_tool_result("tc-1", "not found", true, None);
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["status"], "failed");
    }

    #[test]
    fn tool_result_stale_maps_to_failed_with_reason() {
        let md = ToolResultMetadata::stale(StaleReason::WatchdogTimeout);
        let update = translate_tool_result("tc-1", "no response", true, Some(&md));
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["status"], "failed");
        let out = val["rawOutput"].as_str().unwrap();
        assert!(out.contains("Stale"));
        assert!(out.contains("watchdog timeout"));
    }

    #[test]
    fn tool_result_with_bytes_written_metadata_passes_through() {
        let md = ToolResultMetadata::bytes_written(100);
        let update = translate_tool_result("tc-1", "ok", false, Some(&md));
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["status"], "completed");
    }

    #[test]
    fn tool_result_cancel_maps_to_failed_with_cause() {
        let md = ToolResultMetadata::cancelled(CancelCause::UserInterrupt);
        let update = translate_tool_result("tc-1", "Interrupted by user", true, Some(&md));
        let val = serde_json::to_value(&update).unwrap();
        assert_eq!(val["status"], "failed");
        let out = val["rawOutput"].as_str().unwrap();
        assert!(out.contains("Cancelled"));
        assert!(out.contains("user interrupt"));
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
