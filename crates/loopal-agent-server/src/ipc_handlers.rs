use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info, warn};
use uuid::Uuid;

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::rpc_error::RpcError;
use loopal_protocol::{DEFAULT_INTERACTION_RPC_TIMEOUT, Question, UserQuestionResponse};
use loopal_runtime::frontend::permission_handler::{PermissionHandler, PermissionOutcome};
use loopal_runtime::frontend::question_handler::{AskOptions, QuestionHandler, QuestionOutcome};
use loopal_runtime::frontend::traits::{PlanApproval, PlanApprovalCancellationReason};

use crate::session_hub::SharedSession;

pub type SessionRef = Arc<tokio::sync::RwLock<Arc<SharedSession>>>;

#[derive(Debug)]
enum InteractionRpcError {
    TimedOut,
    Rpc(RpcError),
}

impl std::fmt::Display for InteractionRpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimedOut => formatter.write_str("interaction RPC timed out"),
            Self::Rpc(error) => error.fmt(formatter),
        }
    }
}

async fn send_interaction_request(
    connection: &Connection<Listening>,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, InteractionRpcError> {
    tokio::time::timeout(timeout, connection.send_request(method, params))
        .await
        .map_err(|_| InteractionRpcError::TimedOut)?
        .map_err(InteractionRpcError::Rpc)
}

async fn primary_connection(session: &SessionRef) -> Option<Arc<Connection<Listening>>> {
    let snap = session.read().await.clone();
    snap.primary_connection().await
}

pub struct IpcPermissionHandler {
    session: SessionRef,
    request_timeout: Duration,
}

impl IpcPermissionHandler {
    pub fn new(session: SessionRef) -> Self {
        Self {
            session,
            request_timeout: DEFAULT_INTERACTION_RPC_TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_timeout(session: SessionRef, request_timeout: Duration) -> Self {
        Self {
            session,
            request_timeout,
        }
    }
}

#[async_trait]
impl PermissionHandler for IpcPermissionHandler {
    async fn decide(&self, id: &str, name: &str, input: &serde_json::Value) -> PermissionOutcome {
        info!(tool = name, "requesting permission via IPC");
        let Some(conn) = primary_connection(&self.session).await else {
            warn!(tool = name, "permission denied: no primary connection");
            return PermissionOutcome::deny("no primary connection");
        };
        let params = serde_json::json!({
            "tool_call_id": id,
            "tool_name": name,
            "tool_input": input,
        });
        match send_interaction_request(
            &conn,
            methods::AGENT_PERMISSION.name,
            params,
            self.request_timeout,
        )
        .await
        {
            Ok(value) => {
                let allow = value.get("allow").and_then(Value::as_bool).unwrap_or(false);
                info!(tool = name, allow, "permission response received");
                if allow {
                    PermissionOutcome::allow()
                } else {
                    PermissionOutcome::deny("user denied")
                }
            }
            Err(e) => {
                warn!(tool = name, error = %e, "permission IPC failed");
                PermissionOutcome::deny(format!("ipc failure: {e}"))
            }
        }
    }
}

pub struct IpcQuestionHandler {
    session: SessionRef,
    request_timeout: Duration,
}

impl IpcQuestionHandler {
    pub fn new(session: SessionRef) -> Self {
        Self {
            session,
            request_timeout: DEFAULT_INTERACTION_RPC_TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_timeout(session: SessionRef, request_timeout: Duration) -> Self {
        Self {
            session,
            request_timeout,
        }
    }
}

#[async_trait]
impl QuestionHandler for IpcQuestionHandler {
    async fn ask(&self, questions: Vec<Question>) -> QuestionOutcome {
        self.ask_with_options(questions, AskOptions::manual(String::new()))
            .await
    }

    async fn ask_with_options(
        &self,
        questions: Vec<Question>,
        options: AskOptions,
    ) -> QuestionOutcome {
        debug!(count = questions.len(), "asking user via hub");
        let Some(conn) = primary_connection(&self.session).await else {
            return QuestionOutcome::cancelled("", "no primary connection");
        };
        let mut params = serde_json::json!({ "questions": questions });
        if !options.id.is_empty() {
            params["question_id"] = serde_json::Value::String(options.id.clone());
        }
        if options.classifier_running {
            params["classifier_running"] = serde_json::Value::Bool(true);
        }
        match send_interaction_request(
            &conn,
            methods::AGENT_QUESTION.name,
            params,
            self.request_timeout,
        )
        .await
        {
            Ok(value) => match serde_json::from_value::<UserQuestionResponse>(value) {
                Ok(resp) => QuestionOutcome::manual(resp),
                Err(e) => QuestionOutcome::cancelled("", format!("response parse error: {e}")),
            },
            Err(e) => {
                warn!(error = %e, "ask_user IPC failed");
                QuestionOutcome::cancelled(&options.id, format!("ipc failure: {e}"))
            }
        }
    }
}

pub async fn request_plan_approval(
    session: &SessionRef,
    plan_content: &str,
    plan_path: &str,
) -> PlanApproval {
    request_plan_approval_with_timeout(
        session,
        plan_content,
        plan_path,
        DEFAULT_INTERACTION_RPC_TIMEOUT,
    )
    .await
}

async fn request_plan_approval_with_timeout(
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
        Some("cancelled") => PlanApproval::Cancelled(parse_plan_cancellation_reason(&value)),
        Some("approve_with_edits") => value
            .get("edited_plan")
            .and_then(Value::as_str)
            .map(|value| PlanApproval::ApproveWithEdits(value.to_string()))
            .unwrap_or(PlanApproval::Reject),
        _ => PlanApproval::Reject,
    }
}

fn parse_plan_cancellation_reason(value: &Value) -> PlanApprovalCancellationReason {
    match value.get("reason").and_then(Value::as_str) {
        Some("interrupted") => PlanApprovalCancellationReason::Interrupted,
        Some("timed_out") => PlanApprovalCancellationReason::TimedOut,
        Some("superseded") => PlanApprovalCancellationReason::Superseded,
        Some("transport") => PlanApprovalCancellationReason::Transport,
        _ => PlanApprovalCancellationReason::Unavailable,
    }
}

#[cfg(test)]
#[path = "ipc_handlers/tests.rs"]
mod tests;
