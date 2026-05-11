use async_trait::async_trait;

use loopal_protocol::{Question, UserQuestionResponse};

pub const QUESTION_RESPONSE_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(600);

#[derive(Debug, Clone)]
pub struct QuestionOutcome {
    pub response: UserQuestionResponse,
    pub reason: String,
    pub duration_ms: u64,
}

impl QuestionOutcome {
    pub fn manual(response: UserQuestionResponse) -> Self {
        Self {
            response,
            reason: String::new(),
            duration_ms: 0,
        }
    }

    pub fn cancelled(id: &str, reason: impl Into<String>) -> Self {
        Self {
            response: UserQuestionResponse::cancelled(id),
            reason: reason.into(),
            duration_ms: 0,
        }
    }
}

#[async_trait]
pub trait QuestionHandler: Send + Sync {
    async fn ask(&self, questions: Vec<Question>) -> QuestionOutcome;
}
