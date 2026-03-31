use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tracing::warn;

use loopal_protocol::{AgentEvent, AgentEventPayload, Question, UserQuestionResponse};

/// Handler for AskUser questions — relays to an external consumer, waits for answer.
#[async_trait]
pub trait QuestionHandler: Send + Sync {
    async fn ask(&self, questions: Vec<Question>) -> Vec<String>;
}

/// Relay question handler: emits event, waits on response channel.
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
    async fn ask(&self, questions: Vec<Question>) -> Vec<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let event = AgentEvent::root(AgentEventPayload::UserQuestionRequest { id, questions });
        if self.event_tx.send(event).await.is_err() {
            warn!("question event channel closed");
            return vec!["(channel closed)".into()];
        }
        let mut rx = self.response_rx.lock().await;
        match rx.recv().await {
            Some(response) => response.answers,
            None => vec!["(no response)".into()],
        }
    }
}

/// Auto-cancel handler for sub-agents (questions not supported).
pub struct AutoCancelQuestionHandler;

#[async_trait]
impl QuestionHandler for AutoCancelQuestionHandler {
    async fn ask(&self, _questions: Vec<Question>) -> Vec<String> {
        vec!["(not supported in sub-agent)".into()]
    }
}
