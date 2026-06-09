use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tracing::{info, warn};
use uuid::Uuid;

use loopal_classifier::ClassifierEngine;
use loopal_protocol::Question;
use loopal_provider_api::{ProviderResolver, TaskType};

use super::super::decision_cell::DecisionCell;
use super::super::decision_context::DecisionContext;
use super::super::question_handler::{QuestionHandler, QuestionOutcome};
use super::super::traits::EventEmitter;
use super::classifier_race::run_race;
use super::classifier_task::ClassifierTaskCtx;

pub struct ClassifierQuestionHandler {
    pub(super) classifier: Arc<ClassifierEngine>,
    // reason: fallback is expected to be a leaf handler (Manual / IPC). The
    // production factory wires it to a single IpcQuestionHandler; do NOT chain
    // ClassifierQuestionHandler instances as their own fallback or you'll get
    // recursive race spawning. Future AgentQuestionHandler will fall back here,
    // forming at most a 2-deep chain (Agent → Classifier → Manual).
    pub(super) fallback: Arc<dyn QuestionHandler>,
    pub(super) resolver: Arc<dyn ProviderResolver>,
    pub(super) context: DecisionContext,
    pub(super) emitter: Arc<dyn EventEmitter>,
    pub(super) progress_interval: Duration,
    pub(super) decision: DecisionCell,
}

impl ClassifierQuestionHandler {
    pub fn new(
        classifier: Arc<ClassifierEngine>,
        fallback: Arc<dyn QuestionHandler>,
        resolver: Arc<dyn ProviderResolver>,
        context: DecisionContext,
        emitter: Arc<dyn EventEmitter>,
    ) -> Self {
        Self {
            classifier,
            fallback,
            resolver,
            context,
            emitter,
            progress_interval: Duration::from_millis(500),
            decision: DecisionCell::new(loopal_decision_api::DecisionMode::Classifier),
        }
    }

    pub fn with_decision(mut self, decision: DecisionCell) -> Self {
        self.decision = decision;
        self
    }

    pub fn with_progress_interval(mut self, d: Duration) -> Self {
        self.progress_interval = d;
        self
    }

    pub(super) fn classifier_ctx(&self) -> ClassifierTaskCtx {
        ClassifierTaskCtx {
            classifier: self.classifier.clone(),
            resolver: self.resolver.clone(),
            context: self.context.clone(),
        }
    }
}

#[async_trait]
impl QuestionHandler for ClassifierQuestionHandler {
    async fn ask(&self, questions: Vec<Question>) -> QuestionOutcome {
        if self.decision.get() == loopal_decision_api::DecisionMode::Manual {
            return self.fallback.ask(questions).await;
        }
        if self.classifier.is_degraded() {
            warn!("classifier degraded; pure manual");
            return self.fallback.ask(questions).await;
        }
        if self.resolver.resolve_for(TaskType::Classification).is_err() {
            warn!("classifier provider unavailable; pure manual");
            return self.fallback.ask(questions).await;
        }
        let question_id = Uuid::new_v4().to_string();
        let outcome = run_race(self, question_id, questions).await;
        info!(
            source = outcome.source.as_str(),
            duration_ms = outcome.duration_ms,
            "classifier-question decided"
        );
        outcome
    }
}
