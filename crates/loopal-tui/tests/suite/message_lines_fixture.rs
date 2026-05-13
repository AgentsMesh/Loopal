use std::time::{Duration, Instant};

use loopal_view_state::{
    FailureKind, InvocationId, InvocationState, Outcome, SessionMessage, ToolInvocation,
};

pub fn msg(role: &str, content: &str) -> SessionMessage {
    SessionMessage {
        role: role.to_string(),
        content: content.to_string(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
        inbox: None,
        message_id: None,
        ui_local: false,
    }
}

pub fn all_text(lines: &[ratatui::prelude::Line<'_>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn done_success(content: &str) -> InvocationState {
    InvocationState::Done {
        duration: Duration::from_millis(100),
        outcome: Outcome::Success {
            content: content.to_string(),
        },
    }
}

pub fn done_failure(error: &str) -> InvocationState {
    InvocationState::Done {
        duration: Duration::from_millis(200),
        outcome: Outcome::Failure {
            error: error.to_string(),
            kind: FailureKind::ToolError,
        },
    }
}

pub fn pending_call(name: &str, summary: &str) -> ToolInvocation {
    ToolInvocation::start(
        InvocationId::new("tc-1").unwrap(),
        name,
        summary,
        None,
        Instant::now(),
    )
}

pub fn stale_state(reason: loopal_view_state::StaleReason) -> InvocationState {
    InvocationState::Stale {
        duration: Duration::from_secs(5),
        reason,
    }
}

pub fn cancelled_state(cause: loopal_view_state::CancelCause) -> InvocationState {
    InvocationState::Cancelled {
        duration: Duration::from_secs(2),
        cause,
    }
}
