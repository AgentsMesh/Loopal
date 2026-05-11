use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use loopal_auto_mode::AutoClassifier;
use loopal_protocol::{Question, UserQuestionResponse};
use loopal_provider_api::{ProviderResolver, TaskType};

use super::super::decision_context::DecisionContext;
use super::super::question_handler::{QuestionHandler, QuestionOutcome};

pub struct AutoQuestionHandler {
    classifier: Arc<AutoClassifier>,
    fallback: Box<dyn QuestionHandler>,
    resolver: Arc<dyn ProviderResolver>,
    context: DecisionContext,
}

impl AutoQuestionHandler {
    pub fn new(
        classifier: Arc<AutoClassifier>,
        fallback: Box<dyn QuestionHandler>,
        resolver: Arc<dyn ProviderResolver>,
        context: DecisionContext,
    ) -> Self {
        Self {
            classifier,
            fallback,
            resolver,
            context,
        }
    }

    async fn fall_back(
        &self,
        reason: String,
        duration_ms: u64,
        questions: Vec<Question>,
    ) -> QuestionOutcome {
        let o = self.fallback.ask(questions).await;
        let combined_reason = if o.reason.is_empty() {
            reason
        } else {
            format!("{reason} (fallback: {})", o.reason)
        };
        let combined_duration = if o.duration_ms == 0 {
            duration_ms
        } else {
            o.duration_ms
        };
        QuestionOutcome {
            response: o.response,
            reason: combined_reason,
            duration_ms: combined_duration,
        }
    }
}

#[async_trait]
impl QuestionHandler for AutoQuestionHandler {
    async fn ask(&self, questions: Vec<Question>) -> QuestionOutcome {
        if self.classifier.is_degraded() {
            warn!("auto question classifier degraded");
            return self
                .fall_back("classifier degraded".into(), 0, questions)
                .await;
        }
        let (model, provider) = match self.resolver.resolve_for(TaskType::Classification) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "auto-question provider lookup failed");
                return self
                    .fall_back(format!("provider lookup failed: {e}"), 0, questions)
                    .await;
            }
        };
        let context = self.context.recent().await;
        let cwd = self.context.cwd();
        let result = self
            .classifier
            .classify_question(&questions, &context, cwd, provider.as_ref(), &model)
            .await;
        if let Some(err) = &result.error {
            warn!(error = %err, "auto-question failed");
            return self
                .fall_back(
                    format!("classifier error: {err}"),
                    result.duration_ms,
                    questions,
                )
                .await;
        }
        if result.answers.len() != questions.len() {
            warn!(
                expected = questions.len(),
                got = result.answers.len(),
                "auto-question answer count mismatch"
            );
            return self
                .fall_back(
                    "answer count mismatch".into(),
                    result.duration_ms,
                    questions,
                )
                .await;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let flat: Vec<String> = result
            .answers
            .iter()
            .map(|labels| labels.join(", "))
            .collect();
        info!(
            duration_ms = result.duration_ms,
            reason = %result.reason,
            "auto-question decided"
        );
        QuestionOutcome {
            response: UserQuestionResponse::answered(&id, flat),
            reason: result.reason,
            duration_ms: result.duration_ms,
        }
    }
}
