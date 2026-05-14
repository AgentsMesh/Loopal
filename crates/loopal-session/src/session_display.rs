use std::time::{Duration, Instant};

use loopal_protocol::ProjectedMessage;
use loopal_view_state::{
    FailureKind, InvocationId, InvocationState, Outcome, SessionMessage, StaleReason,
    ToolInvocation, ToolResultMetadata,
};

pub fn into_session_message(p: ProjectedMessage) -> SessionMessage {
    let now = Instant::now();
    SessionMessage {
        role: p.role,
        content: p.content,
        tool_calls: p
            .tool_calls
            .into_iter()
            .filter_map(|tc| {
                let id = InvocationId::new(tc.id).ok()?;
                let state = derive_state(&tc.result, tc.is_error, tc.metadata.as_ref());
                Some(ToolInvocation {
                    id,
                    name: tc.name,
                    summary: tc.summary,
                    input: tc.input,
                    started_at: now,
                    state,
                    batch_id: None,
                    metadata: tc.metadata,
                })
            })
            .collect(),
        image_count: p.image_count,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    }
}

fn derive_state(
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
