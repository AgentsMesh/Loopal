use std::sync::Arc;

use loopal_classifier::{ClassifierEngine, QuestionResult};
use loopal_protocol::Question;
use loopal_provider_api::{ProviderResolver, TaskType};

use super::super::decision_context::DecisionContext;

pub(super) struct ClassifierTaskCtx {
    pub(super) classifier: Arc<ClassifierEngine>,
    pub(super) resolver: Arc<dyn ProviderResolver>,
    pub(super) context: DecisionContext,
}

impl ClassifierTaskCtx {
    pub(super) async fn run(
        &self,
        questions: Vec<Question>,
    ) -> Result<(QuestionResult, usize), String> {
        let (model, provider) = self
            .resolver
            .resolve_for(TaskType::Classification)
            .map_err(|e| format!("provider lookup failed: {e}"))?;
        let context_recent = self.context.recent().await;
        let cwd = self.context.cwd().to_string();
        let expected = questions.len();
        let r = self
            .classifier
            .classify_question(&questions, &context_recent, &cwd, provider.as_ref(), &model)
            .await;
        Ok((r, expected))
    }
}
