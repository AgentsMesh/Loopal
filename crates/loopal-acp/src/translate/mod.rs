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
/// Three-way dispatch: standard events → `SessionUpdate`; Loopal-specific
/// events → `Extension` (`_loopal/*`); events with no IDE counterpart → `None`.
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
        AgentEventPayload::SessionResumeWarnings {
            session_id: warned_session,
            warnings,
        } => {
            let (method, params) = ext::session_resume_warnings(warned_session, warnings);
            Some(AcpNotification::Extension { method, params })
        }
        AgentEventPayload::SessionResumed {
            session_id: resumed_session,
            message_count,
        } => {
            let (method, params) = ext::session_resumed(resumed_session, *message_count);
            Some(AcpNotification::Extension { method, params })
        }
        AgentEventPayload::InboxEnqueued {
            message_id,
            source,
            content,
            summary,
        } => {
            if source.is_optimistically_rendered() {
                return None;
            }
            let (method, params) =
                ext::inbox_enqueued(session_id, message_id, source, content, summary.as_deref());
            Some(AcpNotification::Extension { method, params })
        }
        AgentEventPayload::InboxConsumed { message_id } => {
            let (method, params) = ext::inbox_consumed(session_id, message_id);
            Some(AcpNotification::Extension { method, params })
        }

        // ── Events with no ACP counterpart ───────────────────────────
        AgentEventPayload::AwaitingInput
        | AgentEventPayload::AutoContinuation { .. }
        | AgentEventPayload::Started
        | AgentEventPayload::Running
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
        | AgentEventPayload::SubAgentSpawned { .. }
        | AgentEventPayload::AutoModeDecision { .. }
        | AgentEventPayload::TurnCompleted { .. }
        | AgentEventPayload::McpStatusReport { .. }
        | AgentEventPayload::BgTaskSpawned { .. }
        | AgentEventPayload::BgTaskOutput { .. }
        | AgentEventPayload::BgTaskCompleted { .. }
        | AgentEventPayload::TasksChanged { .. }
        | AgentEventPayload::CronsChanged { .. }
        | AgentEventPayload::UserMessageQueued { .. }
        | AgentEventPayload::ThreadGoalUpdated { .. } => None,
        AgentEventPayload::ToolPermissionResolved { id } => Some(AcpNotification::Extension {
            method: "_loopal/permission_resolved".into(),
            params: serde_json::json!({
                "sessionId": session_id,
                "toolCallId": id,
            }),
        }),
        AgentEventPayload::UserQuestionResolved { id } => Some(AcpNotification::Extension {
            method: "_loopal/question_resolved".into(),
            params: serde_json::json!({
                "sessionId": session_id,
                "questionId": id,
            }),
        }),
    }
}

pub use tool_kind::map_tool_kind;
