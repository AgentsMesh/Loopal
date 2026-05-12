use loopal_protocol::{AgentEventPayload, UserQuestionResponse};
use loopal_tool_api::PermissionDecision;

use super::permission_handler::PermissionOutcome;
use super::question_handler::QuestionOutcome;

pub fn into_permission_decided(
    name: &str,
    outcome: PermissionOutcome,
) -> (PermissionDecision, AgentEventPayload) {
    let payload = AgentEventPayload::PermissionDecided {
        tool_name: name.into(),
        decision: outcome.decision.as_str().into(),
        reason: outcome.reason,
        duration_ms: outcome.duration_ms,
    };
    (outcome.decision, payload)
}

pub fn into_question_decided(
    question_count: u32,
    outcome: QuestionOutcome,
) -> (UserQuestionResponse, AgentEventPayload) {
    let payload = AgentEventPayload::QuestionDecided {
        question_count,
        duration_ms: outcome.duration_ms,
        reason: outcome.reason,
        source: outcome.source,
    };
    (outcome.response, payload)
}
