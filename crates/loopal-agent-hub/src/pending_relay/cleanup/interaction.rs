use loopal_protocol::{
    AgentEvent, AgentEventPayload, QualifiedAddress, ResolveSource, UserQuestionResponse,
};
use tracing::info;

use super::super::completion::{TerminalEventSink, complete_detached};
use super::super::types::{InteractionAudience, OriginRemoteQuestionCancel};
use super::super::types::{PendingPermissionInfo, PendingPlanApprovalInfo, PendingQuestionInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CleanupReason {
    AgentDisconnected,
    AgentInterrupted,
    NoCapableUi,
    RequestCancelled,
    TimedOut,
}

pub(super) enum PendingInteraction {
    Permission {
        info: PendingPermissionInfo,
    },
    Question {
        id: String,
        info: PendingQuestionInfo,
    },
    PlanApproval {
        info: PendingPlanApprovalInfo,
    },
}

impl PendingInteraction {
    fn remote_cancel(&self) -> Option<OriginRemoteQuestionCancel> {
        let Self::Question { info, .. } = self else {
            return None;
        };
        let InteractionAudience::RemoteUi { target_hub, uplink } = &info.audience else {
            return None;
        };
        Some(OriginRemoteQuestionCancel {
            target_hub: target_hub.clone(),
            origin_agent: info.agent_name.clone(),
            interaction_id: info.interaction_id.clone(),
            uplink: uplink.clone(),
        })
    }

    pub(super) fn resolve(
        self,
        terminal_sink: TerminalEventSink,
        reason: CleanupReason,
        emit_resolved: bool,
    ) {
        let (agent_conn, agent_ipc_id, agent_name, response, event) = match self {
            Self::Permission { info } => (
                info.agent_conn,
                info.agent_ipc_id,
                info.agent_name.clone(),
                serde_json::json!({"allow": false}),
                AgentEvent::named(
                    QualifiedAddress::local(&info.agent_name),
                    AgentEventPayload::ToolPermissionResolved {
                        id: info.interaction_id,
                    },
                ),
            ),
            Self::Question { id, info } => (
                info.agent_conn,
                info.agent_ipc_id,
                info.agent_name.clone(),
                serde_json::to_value(UserQuestionResponse::cancelled(id.clone()))
                    .unwrap_or(serde_json::Value::Null),
                AgentEvent::named(
                    QualifiedAddress::local(&info.agent_name),
                    AgentEventPayload::UserQuestionResolved {
                        id: info.interaction_id,
                        by: ResolveSource::Agent,
                    },
                ),
            ),
            Self::PlanApproval { info } => (
                info.agent_conn,
                info.agent_ipc_id,
                info.agent_name.clone(),
                serde_json::json!({
                    "decision": "cancelled",
                    "reason": plan_cancellation_reason(reason),
                }),
                AgentEvent::named(
                    QualifiedAddress::local(&info.agent_name),
                    AgentEventPayload::PlanApprovalResolved {
                        id: info.interaction_id,
                    },
                ),
            ),
        };

        info!(agent = %agent_name, agent_ipc_id, ?reason, "pending interaction cleaned up");
        let resolved_event = emit_resolved.then_some((terminal_sink, event));
        complete_detached(agent_conn, agent_ipc_id, response, resolved_event);
    }
}

fn plan_cancellation_reason(reason: CleanupReason) -> &'static str {
    match reason {
        CleanupReason::AgentDisconnected => "transport",
        CleanupReason::AgentInterrupted | CleanupReason::RequestCancelled => "interrupted",
        CleanupReason::NoCapableUi => "unavailable",
        CleanupReason::TimedOut => "timed_out",
    }
}

pub(super) fn resolve_all(
    pending: Vec<PendingInteraction>,
    terminal_sink: TerminalEventSink,
    reason: CleanupReason,
    emit_resolved: bool,
    notify_remote: bool,
) {
    for interaction in pending {
        let remote_cancel = notify_remote.then(|| interaction.remote_cancel()).flatten();
        interaction.resolve(terminal_sink.clone(), reason, emit_resolved);
        if let Some(cancel) = remote_cancel {
            crate::remote_relay::cancel_destination_detached(cancel);
        }
    }
}
