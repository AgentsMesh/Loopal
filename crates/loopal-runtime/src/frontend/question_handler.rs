use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::warn;

use loopal_protocol::{AgentEvent, AgentEventPayload, Question, UserQuestionResponse};

/// Maximum time to wait for a user question response before treating as
/// cancel. Long enough to cover human deliberation; short enough to prevent
/// indefinite hangs if the IPC peer dies silently.
pub const QUESTION_RESPONSE_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(600);

#[async_trait]
pub trait QuestionHandler: Send + Sync {
    async fn ask(&self, questions: Vec<Question>) -> UserQuestionResponse;
}

pub struct RelayQuestionHandler {
    event_tx: mpsc::Sender<AgentEvent>,
    response_rx: Mutex<mpsc::Receiver<UserQuestionResponse>>,
}

impl RelayQuestionHandler {
    pub fn new(
        event_tx: mpsc::Sender<AgentEvent>,
        response_rx: mpsc::Receiver<UserQuestionResponse>,
    ) -> Self {
        Self {
            event_tx,
            response_rx: Mutex::new(response_rx),
        }
    }
}

#[async_trait]
impl QuestionHandler for RelayQuestionHandler {
    async fn ask(&self, questions: Vec<Question>) -> UserQuestionResponse {
        let id = uuid::Uuid::new_v4().to_string();
        let event = AgentEvent::root(AgentEventPayload::UserQuestionRequest {
            id: id.clone(),
            questions,
        });
        if self.event_tx.send(event).await.is_err() {
            warn!("question event channel closed");
            return UserQuestionResponse::cancelled(&id);
        }
        let mut rx = self.response_rx.lock().await;
        let timeout = QUESTION_RESPONSE_TIMEOUT;
        loop {
            match tokio::time::timeout(timeout, rx.recv()).await {
                Ok(Some(response)) => {
                    let resp_id = response.question_id();
                    if resp_id.is_empty() {
                        // Frontend self-sentinel (IPC fallback constructs cancelled with
                        // empty id). Treat as the local id and accept; do not loop.
                        warn!(
                            local = %id,
                            "received question response with empty question_id; \
                             treating as frontend self-sentinel and accepting"
                        );
                        return rewrite_question_id(response, &id);
                    }
                    if resp_id != id {
                        warn!(
                            expected = %id,
                            got = %resp_id,
                            "stale question response, discarding"
                        );
                        continue;
                    }
                    return response;
                }
                Ok(None) => return UserQuestionResponse::cancelled(&id),
                Err(_) => {
                    warn!(local = %id, "question response timeout, treating as cancel");
                    while let Ok(stale) = rx.try_recv() {
                        warn!(stale_id = %stale.question_id(), "draining stale response after timeout");
                    }
                    return UserQuestionResponse::cancelled(&id);
                }
            }
        }
    }
}

fn rewrite_question_id(response: UserQuestionResponse, id: &str) -> UserQuestionResponse {
    match response {
        UserQuestionResponse::Answered { answers, .. } => {
            UserQuestionResponse::answered(id, answers)
        }
        UserQuestionResponse::Cancelled { .. } => UserQuestionResponse::cancelled(id),
        UserQuestionResponse::Unsupported { reason, .. } => {
            UserQuestionResponse::unsupported(id, reason)
        }
    }
}

pub struct AutoCancelQuestionHandler;

#[async_trait]
impl QuestionHandler for AutoCancelQuestionHandler {
    async fn ask(&self, _questions: Vec<Question>) -> UserQuestionResponse {
        UserQuestionResponse::unsupported("", "AskUser not supported in this context")
    }
}
