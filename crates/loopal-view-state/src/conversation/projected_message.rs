use std::time::{Duration, Instant};

use loopal_protocol::ProjectedMessage;
use loopal_tool_invocation::{
    FailureKind, InvocationId, InvocationState, Outcome, StaleReason, ToolInvocation,
    ToolResultMetadata,
};

use super::SessionMessage;

pub fn into_session_message(projected: ProjectedMessage) -> SessionMessage {
    let now = Instant::now();
    SessionMessage {
        role: projected.role,
        content: projected.content,
        tool_calls: projected
            .tool_calls
            .into_iter()
            .filter_map(|call| {
                let id = InvocationId::new(call.id).ok()?;
                let state = invocation_state(&call.result, call.is_error, call.metadata.as_ref());
                Some(ToolInvocation {
                    id,
                    name: call.name,
                    summary: call.summary,
                    input: call.input,
                    started_at: now,
                    state,
                    batch_id: None,
                    metadata: call.metadata,
                })
            })
            .collect(),
        image_count: projected.image_count,
        skill_info: projected.skill_info,
        inbox: None,
        message_id: None,
        ui_local: false,
    }
}

fn invocation_state(
    result: &Option<String>,
    is_error: bool,
    metadata: Option<&ToolResultMetadata>,
) -> InvocationState {
    match metadata {
        Some(ToolResultMetadata::Stale { reason }) => InvocationState::Stale {
            duration: Duration::ZERO,
            reason: *reason,
        },
        Some(ToolResultMetadata::Cancelled { cause }) => InvocationState::Cancelled {
            duration: Duration::ZERO,
            cause: *cause,
        },
        _ => match (result, is_error) {
            (Some(content), true) => InvocationState::Done {
                duration: Duration::ZERO,
                outcome: Outcome::Failure {
                    error: content.clone(),
                    kind: FailureKind::ToolError,
                },
            },
            (Some(content), false) => InvocationState::Done {
                duration: Duration::ZERO,
                outcome: Outcome::Success {
                    content: content.clone(),
                },
            },
            (None, _) => InvocationState::Stale {
                duration: Duration::ZERO,
                reason: StaleReason::ConnectionLost,
            },
        },
    }
}
