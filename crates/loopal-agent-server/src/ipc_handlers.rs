use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, warn};

use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::rpc_error::RpcError;
use loopal_protocol::{DEFAULT_INTERACTION_RPC_TIMEOUT, Question, UserQuestionResponse};
use loopal_runtime::frontend::question_handler::{AskOptions, QuestionHandler, QuestionOutcome};

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

mod permission;
mod plan;

pub use permission::IpcPermissionHandler;
pub use plan::request_plan_approval;

#[cfg(test)]
use permission::permission_outcome_from_response;
#[cfg(test)]
#[path = "ipc_handlers/permission_response_tests.rs"]
mod permission_response_tests;
#[cfg(test)]
#[path = "ipc_handlers/plan_tests.rs"]
mod plan_tests;
#[cfg(test)]
#[path = "ipc_handlers/tests.rs"]
mod tests;
