use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::warn;

use loopal_protocol::{AgentEvent, AgentEventPayload, Question, UserQuestionResponse};

use super::super::question_handler::{QUESTION_RESPONSE_TIMEOUT, QuestionHandler, QuestionOutcome};

pub struct ManualQuestionHandler {
    event_tx: mpsc::Sender<AgentEvent>,
    response_rx: Mutex<mpsc::Receiver<UserQuestionResponse>>,
}

impl ManualQuestionHandler {
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
impl QuestionHandler for ManualQuestionHandler {
    async fn ask(&self, questions: Vec<Question>) -> QuestionOutcome {
        let id = uuid::Uuid::new_v4().to_string();
        let event = AgentEvent::root(AgentEventPayload::UserQuestionRequest {
            id: id.clone(),
            questions,
            classifier_running: false,
        });
        if self.event_tx.send(event).await.is_err() {
            warn!("question event channel closed");
            return QuestionOutcome::cancelled(&id, "event channel closed");
        }
        let mut rx = self.response_rx.lock().await;
        loop {
            match tokio::time::timeout(QUESTION_RESPONSE_TIMEOUT, rx.recv()).await {
                Ok(Some(response)) => {
                    let resp_id = response.question_id();
                    if resp_id.is_empty() {
                        warn!(
                            local = %id,
                            "received question response with empty question_id; \
                             treating as frontend self-sentinel and accepting"
                        );
                        return QuestionOutcome::manual(rewrite_question_id(response, &id));
                    }
                    if resp_id != id {
                        warn!(
                            expected = %id,
                            got = %resp_id,
                            "stale question response, discarding"
                        );
                        continue;
                    }
                    return QuestionOutcome::manual(response);
                }
                Ok(None) => {
                    return QuestionOutcome::cancelled(&id, "response channel closed");
                }
                Err(_) => {
                    warn!(local = %id, "question response timeout, treating as cancel");
                    while let Ok(stale) = rx.try_recv() {
                        warn!(stale_id = %stale.question_id(), "draining stale response after timeout");
                    }
                    return QuestionOutcome::cancelled(&id, "user did not answer in time");
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
