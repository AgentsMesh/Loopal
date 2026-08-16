use std::time::Duration;

use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

use loopal_ipc::protocol::methods;
use loopal_protocol::DEFAULT_INTERACTION_RPC_TIMEOUT;
use loopal_runtime::frontend::traits::{PlanApproval, PlanApprovalCancellationReason};

use super::{InteractionRpcError, SessionRef, primary_connection, send_interaction_request};

pub async fn request_plan_approval(
    session: &SessionRef,
    plan_content: &str,
    plan_path: &str,
) -> PlanApproval {
    request_plan_approval_with_timeout(
        session,
        plan_content,
        plan_path,
        default_plan_approval_timeout(),
    )
    .await
}

pub(super) fn default_plan_approval_timeout() -> Duration {
    DEFAULT_INTERACTION_RPC_TIMEOUT
}

pub(super) async fn request_plan_approval_with_timeout(
    session: &SessionRef,
    plan_content: &str,
    plan_path: &str,
    request_timeout: Duration,
) -> PlanApproval {
    let Some(conn) = primary_connection(session).await else {
        return PlanApproval::Cancelled(PlanApprovalCancellationReason::Transport);
    };
    let params = serde_json::json!({
        "request_id": Uuid::new_v4().to_string(),
        "plan_content": plan_content,
        "plan_path": plan_path,
    });
    let value = match send_interaction_request(
        &conn,
        methods::AGENT_PLAN_APPROVAL.name,
        params,
        request_timeout,
    )
    .await
    {
        Ok(value) => value,
        Err(InteractionRpcError::TimedOut) => {
            warn!("plan approval IPC timed out");
            return PlanApproval::Cancelled(PlanApprovalCancellationReason::TimedOut);
        }
        Err(error) => {
            warn!(%error, "plan approval IPC failed");
            return PlanApproval::Cancelled(PlanApprovalCancellationReason::Transport);
        }
    };
    match value.get("decision").and_then(Value::as_str) {
        Some("approve") => PlanApproval::Approve,
        Some("cancelled") => PlanApproval::Cancelled(parse_cancellation_reason(&value)),
        Some("approve_with_edits") => value
            .get("edited_plan")
            .and_then(Value::as_str)
            .map(|value| PlanApproval::ApproveWithEdits(value.to_string()))
            .unwrap_or(PlanApproval::Reject),
        _ => PlanApproval::Reject,
    }
}

fn parse_cancellation_reason(value: &Value) -> PlanApprovalCancellationReason {
    match value.get("reason").and_then(Value::as_str) {
        Some("interrupted") => PlanApprovalCancellationReason::Interrupted,
        Some("timed_out") => PlanApprovalCancellationReason::TimedOut,
        Some("superseded") => PlanApprovalCancellationReason::Superseded,
        Some("transport") => PlanApprovalCancellationReason::Transport,
        _ => PlanApprovalCancellationReason::Unavailable,
    }
}
