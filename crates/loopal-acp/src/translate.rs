//! Translate `AgentEventPayload` into ACP `SessionUpdate` notifications.

use loopal_protocol::AgentEventPayload;
use serde_json::Value;
use uuid::Uuid;

use crate::types::{AcpContentBlock, SessionUpdate, SessionUpdateParams, ToolCallStatus, ToolKind};

/// Convert an `AgentEventPayload` into a JSON-serialised `session/update` params value.
///
/// Returns `None` for events that have no ACP counterpart (e.g. `AwaitingInput`,
/// `TokenUsage`).
pub fn translate_event(payload: &AgentEventPayload, session_id: &str) -> Option<Value> {
    let update = match payload {
        AgentEventPayload::Stream { text } => SessionUpdate::AgentMessageChunk {
            message_id: Uuid::new_v4().to_string(),
            content: vec![AcpContentBlock::Text { text: text.clone() }],
        },
        AgentEventPayload::ToolCall { id, name, .. } => SessionUpdate::ToolCall {
            tool_call_id: id.clone(),
            title: name.clone(),
            tool_call_kind: map_tool_kind(name),
            status: ToolCallStatus::Pending,
        },
        AgentEventPayload::ToolResult {
            id,
            result,
            is_error,
            ..
        } => SessionUpdate::ToolCallUpdate {
            tool_call_id: id.clone(),
            status: if *is_error {
                ToolCallStatus::Failed
            } else {
                ToolCallStatus::Completed
            },
            content: Some(result.clone()),
        },
        AgentEventPayload::Error { message } => SessionUpdate::AgentMessageChunk {
            message_id: Uuid::new_v4().to_string(),
            content: vec![AcpContentBlock::Text {
                text: format!("[error] {message}"),
            }],
        },
        AgentEventPayload::ModeChanged { mode } => SessionUpdate::AgentMessageChunk {
            message_id: Uuid::new_v4().to_string(),
            content: vec![AcpContentBlock::Text {
                text: format!("[mode changed: {mode}]"),
            }],
        },
        // Events with no ACP counterpart
        AgentEventPayload::AwaitingInput
        | AgentEventPayload::MaxTurnsReached { .. }
        | AgentEventPayload::TokenUsage { .. }
        | AgentEventPayload::AutoContinuation { .. }
        | AgentEventPayload::Started
        | AgentEventPayload::Finished
        | AgentEventPayload::MessageRouted { .. }
        | AgentEventPayload::ToolPermissionRequest { .. }
        | AgentEventPayload::UserQuestionRequest { .. }
        | AgentEventPayload::ThinkingStream { .. }
        | AgentEventPayload::ThinkingComplete { .. }
        | AgentEventPayload::Rewound { .. }
        | AgentEventPayload::Compacted { .. }
        | AgentEventPayload::Interrupted
        | AgentEventPayload::TurnDiffSummary { .. } => return None,
    };

    let params = SessionUpdateParams {
        session_id: session_id.to_string(),
        update,
    };
    serde_json::to_value(params).ok()
}

/// Map a Loopal tool name to an ACP `ToolKind`.
pub fn map_tool_kind(name: &str) -> ToolKind {
    match name {
        "Read" | "Glob" | "Grep" | "Ls" => ToolKind::Read,
        "Write" | "Edit" | "MultiEdit" | "ApplyPatch" => ToolKind::Edit,
        "Bash" => ToolKind::Execute,
        "WebFetch" | "WebSearch" => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_event_translates_to_chunk() {
        let payload = AgentEventPayload::Stream {
            text: "hello".into(),
        };
        let val = translate_event(&payload, "sess-1").unwrap();
        let update = val["update"].clone();
        assert_eq!(update["kind"], "agent_message_chunk");
        assert_eq!(update["content"][0]["text"], "hello");
    }

    #[test]
    fn tool_call_translates() {
        let payload = AgentEventPayload::ToolCall {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"path": "/foo"}),
        };
        let val = translate_event(&payload, "sess-1").unwrap();
        let update = val["update"].clone();
        assert_eq!(update["kind"], "tool_call");
        assert_eq!(update["tool_call_kind"], "read");
        assert_eq!(update["status"], "pending");
    }

    #[test]
    fn tool_result_success() {
        let payload = AgentEventPayload::ToolResult {
            id: "tc-1".into(),
            name: "Read".into(),
            result: "file contents".into(),
            is_error: false,
        };
        let val = translate_event(&payload, "sess-1").unwrap();
        assert_eq!(val["update"]["status"], "completed");
    }

    #[test]
    fn tool_result_error() {
        let payload = AgentEventPayload::ToolResult {
            id: "tc-1".into(),
            name: "Read".into(),
            result: "not found".into(),
            is_error: true,
        };
        let val = translate_event(&payload, "sess-1").unwrap();
        assert_eq!(val["update"]["status"], "failed");
    }

    #[test]
    fn awaiting_input_returns_none() {
        assert!(translate_event(&AgentEventPayload::AwaitingInput, "s").is_none());
    }

    #[test]
    fn map_tool_kinds() {
        assert!(matches!(map_tool_kind("Read"), ToolKind::Read));
        assert!(matches!(map_tool_kind("Bash"), ToolKind::Execute));
        assert!(matches!(map_tool_kind("Write"), ToolKind::Edit));
        assert!(matches!(map_tool_kind("WebFetch"), ToolKind::Fetch));
        assert!(matches!(map_tool_kind("CustomTool"), ToolKind::Other));
    }
}
