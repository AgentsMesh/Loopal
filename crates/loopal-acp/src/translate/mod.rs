//! Translate `AgentEventPayload` into ACP notifications.
//!
//! Standard events become `session/update` with a `SessionUpdate` payload.
//! Loopal-specific events become extension notifications (`_loopal/*`).

pub(crate) mod ext;
mod messages;
mod tool_kind;
mod tools;

use loopal_protocol::AgentEventPayload;
use serde_json::Value;

use crate::types::make_session_notification;

/// A translated ACP notification ready to send.
pub enum AcpNotification {
    /// Standard `session/update` notification.
    SessionUpdate(Value),
    /// Extension notification with custom method name.
    Extension { method: String, params: Value },
}

/// Convert an `AgentEventPayload` into an ACP notification.
///
/// Returns `None` for events that have no ACP counterpart (e.g. `AwaitingInput`).
pub fn translate_event(payload: &AgentEventPayload, session_id: &str) -> Option<AcpNotification> {
    match payload {
        // ── Message streaming ────────────────────────────────────────
        AgentEventPayload::Stream { text } => {
            let u = messages::translate_stream(text);
            Some(AcpNotification::SessionUpdate(make_session_notification(
                session_id, u,
            )))
        }
        AgentEventPayload::ThinkingStream { text } => {
            let u = messages::translate_thinking(text);
            Some(AcpNotification::SessionUpdate(make_session_notification(
                session_id, u,
            )))
        }
        AgentEventPayload::Error { message } => {
            let u = messages::translate_error(message);
            Some(AcpNotification::SessionUpdate(make_session_notification(
                session_id, u,
            )))
        }
        AgentEventPayload::ModeChanged { mode } => {
            let u = messages::translate_mode_changed(mode);
            Some(AcpNotification::SessionUpdate(make_session_notification(
                session_id, u,
            )))
        }

        // ── Tool lifecycle ───────────────────────────────────────────
        AgentEventPayload::ToolCall { id, name, .. } => {
            let u = tools::translate_tool_call(id, name);
            Some(AcpNotification::SessionUpdate(make_session_notification(
                session_id, u,
            )))
        }
        AgentEventPayload::ToolResult {
            id,
            result,
            is_error,
            ..
        } => {
            let u = tools::translate_tool_result(id, result, *is_error);
            Some(AcpNotification::SessionUpdate(make_session_notification(
                session_id, u,
            )))
        }
        AgentEventPayload::ToolProgress {
            id, output_tail, ..
        } => {
            let u = tools::translate_tool_progress(id, output_tail);
            Some(AcpNotification::SessionUpdate(make_session_notification(
                session_id, u,
            )))
        }

        // ── Extension notifications ──────────────────────────────────
        AgentEventPayload::RetryError {
            message,
            attempt,
            max_attempts,
        } => {
            let (method, params) = ext::retry_error(session_id, message, *attempt, *max_attempts);
            Some(AcpNotification::Extension { method, params })
        }
        AgentEventPayload::TokenUsage { .. } => {
            let usage = serde_json::to_value(payload).unwrap_or_default();
            let (method, params) = ext::token_usage(session_id, &usage);
            Some(AcpNotification::Extension { method, params })
        }

        // ── Events with no ACP counterpart ───────────────────────────
        AgentEventPayload::AwaitingInput
        | AgentEventPayload::MaxTurnsReached { .. }
        | AgentEventPayload::AutoContinuation { .. }
        | AgentEventPayload::Started
        | AgentEventPayload::Finished
        | AgentEventPayload::MessageRouted { .. }
        | AgentEventPayload::ToolPermissionRequest { .. }
        | AgentEventPayload::UserQuestionRequest { .. }
        | AgentEventPayload::ThinkingComplete { .. }
        | AgentEventPayload::Rewound { .. }
        | AgentEventPayload::Compacted { .. }
        | AgentEventPayload::ToolBatchStart { .. }
        | AgentEventPayload::Interrupted
        | AgentEventPayload::TurnDiffSummary { .. }
        | AgentEventPayload::ServerToolUse { .. }
        | AgentEventPayload::ServerToolResult { .. }
        | AgentEventPayload::RetryCleared
        | AgentEventPayload::SubAgentSpawned { .. } => None,
    }
}

pub use tool_kind::map_tool_kind;

#[cfg(test)]
mod tests {
    use super::*;
    use loopal_protocol::AgentEventPayload;

    #[test]
    fn stream_returns_session_update() {
        let r = translate_event(&AgentEventPayload::Stream { text: "hi".into() }, "s");
        assert!(matches!(r, Some(AcpNotification::SessionUpdate(_))));
    }

    #[test]
    fn thinking_returns_session_update() {
        let r = translate_event(&AgentEventPayload::ThinkingStream { text: "t".into() }, "s");
        assert!(matches!(r, Some(AcpNotification::SessionUpdate(_))));
    }

    #[test]
    fn retry_error_returns_extension() {
        let r = translate_event(
            &AgentEventPayload::RetryError {
                message: "e".into(),
                attempt: 1,
                max_attempts: 3,
            },
            "s",
        );
        assert!(matches!(r, Some(AcpNotification::Extension { .. })));
    }

    #[test]
    fn none_events_return_none() {
        let nones = vec![
            AgentEventPayload::AwaitingInput,
            AgentEventPayload::MaxTurnsReached { turns: 50 },
            AgentEventPayload::Started,
            AgentEventPayload::Finished,
            AgentEventPayload::Interrupted,
            AgentEventPayload::RetryCleared,
        ];
        for ev in &nones {
            assert!(
                translate_event(ev, "s").is_none(),
                "expected None for {ev:?}"
            );
        }
    }
}
