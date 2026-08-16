use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use loopal_classifier::ClassifierEngine;
use loopal_protocol::PermissionIntentRequest;
use loopal_provider_api::{ProviderResolver, TaskType};
use loopal_tool_api::PermissionDecision;

use super::super::decision_cell::DecisionCell;
use super::super::decision_context::DecisionContext;
use super::super::degraded::DegradedAction;
use super::super::permission_handler::{PermissionHandler, PermissionOutcome};

pub struct ClassifierPermissionHandler {
    classifier: Arc<ClassifierEngine>,
    fallback: Box<dyn PermissionHandler>,
    resolver: Arc<dyn ProviderResolver>,
    context: DecisionContext,
    decision: DecisionCell,
    on_provider_error: DegradedAction,
}

impl ClassifierPermissionHandler {
    pub fn new(
        classifier: Arc<ClassifierEngine>,
        fallback: Box<dyn PermissionHandler>,
        resolver: Arc<dyn ProviderResolver>,
        context: DecisionContext,
    ) -> Self {
        Self {
            classifier,
            fallback,
            resolver,
            context,
            decision: DecisionCell::new(loopal_decision_api::DecisionMode::Classifier),
            on_provider_error: DegradedAction::Fallback,
        }
    }

    pub fn with_decision(mut self, decision: DecisionCell) -> Self {
        self.decision = decision;
        self
    }

    pub fn with_provider_error_action(mut self, action: DegradedAction) -> Self {
        self.on_provider_error = action;
        self
    }

    async fn fall_back(
        &self,
        reason: String,
        request: &PermissionIntentRequest,
    ) -> PermissionOutcome {
        // reason: when the classifier was degraded and the user manually
        // approves, take that as a "user is engaged" signal and reset the
        // circuit so the classifier can try again on the next request. Without
        // this wire-up the degraded state would persist for the rest of the
        // session even after the user demonstrates they're handling things.
        let was_degraded = self.classifier.is_degraded();
        let o = self.fallback.decide(request).await;
        if was_degraded && o.decision == PermissionDecision::Allow {
            self.classifier.on_human_approval(&request.tool_name);
        }
        let combined_reason = if o.reason.is_empty() {
            reason
        } else {
            format!("{reason} (fallback: {})", o.reason)
        };
        PermissionOutcome {
            reason: combined_reason,
            ..o
        }
    }

    async fn apply_provider_error(
        &self,
        reason: String,
        request: &PermissionIntentRequest,
    ) -> PermissionOutcome {
        match self.on_provider_error {
            DegradedAction::Fallback => self.fall_back(reason, request).await,
            DegradedAction::Deny => PermissionOutcome::deny(reason),
        }
    }
}

#[async_trait]
impl PermissionHandler for ClassifierPermissionHandler {
    async fn decide(&self, request: &PermissionIntentRequest) -> PermissionOutcome {
        if request.intent_seed.workflow().is_some() {
            return self.fallback.decide(request).await;
        }
        let name = &request.tool_name;
        let input = &request.action_input;
        if self.decision.get() == loopal_decision_api::DecisionMode::Manual {
            return self.fallback.decide(request).await;
        }
        if self.classifier.is_degraded() {
            warn!(tool = name, "classifier degraded");
            return self.fall_back("classifier degraded".into(), request).await;
        }
        let (model, provider) = match self.resolver.resolve_for(TaskType::Classification) {
            Ok(p) => p,
            Err(e) => {
                warn!(tool = name, error = %e, "classifier provider lookup failed");
                return self
                    .apply_provider_error(format!("provider lookup failed: {e}"), request)
                    .await;
            }
        };
        let context = self.context.recent().await;
        let cwd = self.context.cwd();
        let result = self
            .classifier
            .classify(name, input, &context, cwd, provider.as_ref(), &model)
            .await;
        info!(tool = name, decision = ?result.decision, reason = %result.reason, "classifier-permission");
        PermissionOutcome {
            decision: result.decision,
            reason: result.reason,
            duration_ms: result.duration_ms,
            receipt: None,
        }
    }
}
