use async_trait::async_trait;

use loopal_protocol::{Question, ResolveSource, UserQuestionResponse};

use super::super::question_handler::{QuestionHandler, QuestionOutcome};

pub struct UnsupportedQuestionHandler;

#[async_trait]
impl QuestionHandler for UnsupportedQuestionHandler {
    async fn ask(&self, _questions: Vec<Question>) -> QuestionOutcome {
        QuestionOutcome {
            response: UserQuestionResponse::unsupported(
                "",
                "AskUser not supported in this context",
            ),
            reason: "sub-agent context".into(),
            duration_ms: 0,
            source: ResolveSource::Manual,
        }
    }
}
