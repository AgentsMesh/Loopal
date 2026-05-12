use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info, warn};

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{Question, UserQuestionResponse};
use loopal_runtime::frontend::permission_handler::{PermissionHandler, PermissionOutcome};
use loopal_runtime::frontend::question_handler::{AskOptions, QuestionHandler, QuestionOutcome};

use crate::session_hub::SharedSession;

pub type SessionRef = Arc<tokio::sync::RwLock<Arc<SharedSession>>>;

async fn primary_connection(session: &SessionRef) -> Option<Arc<Connection>> {
    let snap = session.read().await.clone();
    snap.primary_connection().await
}

pub struct IpcPermissionHandler {
    session: SessionRef,
}

impl IpcPermissionHandler {
    pub fn new(session: SessionRef) -> Self {
        Self { session }
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
        match conn
            .send_request(methods::AGENT_PERMISSION.name, params)
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
}

impl IpcQuestionHandler {
    pub fn new(session: SessionRef) -> Self {
        Self { session }
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
        match conn
            .send_request(methods::AGENT_QUESTION.name, params)
            .await
        {
            Ok(value) => match serde_json::from_value::<UserQuestionResponse>(value) {
                Ok(resp) => QuestionOutcome::manual(resp),
                Err(e) => QuestionOutcome::cancelled("", format!("response parse error: {e}")),
            },
            Err(e) => {
                warn!(error = %e, "ask_user IPC failed");
                QuestionOutcome::cancelled("", format!("ipc failure: {e}"))
            }
        }
    }
}
