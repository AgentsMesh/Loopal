use async_trait::async_trait;

use loopal_protocol::{Question, ResolveSource, UserQuestionResponse};

pub const QUESTION_RESPONSE_TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(600);

#[derive(Debug, Clone)]
pub struct AskOptions {
    pub id: String,
    pub classifier_running: bool,
}

impl AskOptions {
    pub fn manual(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            classifier_running: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuestionOutcome {
    pub response: UserQuestionResponse,
    pub reason: String,
    pub duration_ms: u64,
    pub source: ResolveSource,
}

impl QuestionOutcome {
    pub fn manual(response: UserQuestionResponse) -> Self {
        Self {
            response,
            reason: String::new(),
            duration_ms: 0,
            source: ResolveSource::Manual,
        }
    }

    pub fn cancelled(id: &str, reason: impl Into<String>) -> Self {
        Self {
            response: UserQuestionResponse::cancelled(id),
            reason: reason.into(),
            duration_ms: 0,
            source: ResolveSource::Manual,
        }
    }

    pub fn classifier(response: UserQuestionResponse, reason: String, duration_ms: u64) -> Self {
        Self {
            response,
            reason,
            duration_ms,
            source: ResolveSource::Classifier,
        }
    }
}

#[async_trait]
pub trait QuestionHandler: Send + Sync {
    async fn ask(&self, questions: Vec<Question>) -> QuestionOutcome;

    async fn ask_with_options(
        &self,
        questions: Vec<Question>,
        _options: AskOptions,
    ) -> QuestionOutcome {
        self.ask(questions).await
    }
}
