use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use loopal_auto_mode::AutoClassifier;
use loopal_provider_api::{ProviderResolver, TaskType};

use super::super::decision_context::DecisionContext;
use super::super::degraded::DegradedAction;
use super::super::permission_handler::{PermissionHandler, PermissionOutcome};

pub struct AutoPermissionHandler {
    classifier: Arc<AutoClassifier>,
    fallback: Box<dyn PermissionHandler>,
    resolver: Arc<dyn ProviderResolver>,
    context: DecisionContext,
    on_provider_error: DegradedAction,
}

impl AutoPermissionHandler {
    pub fn new(
        classifier: Arc<AutoClassifier>,
        fallback: Box<dyn PermissionHandler>,
        resolver: Arc<dyn ProviderResolver>,
        context: DecisionContext,
    ) -> Self {
        Self {
            classifier,
            fallback,
            resolver,
            context,
            on_provider_error: DegradedAction::Fallback,
        }
    }

    pub fn with_provider_error_action(mut self, action: DegradedAction) -> Self {
        self.on_provider_error = action;
        self
    }

    async fn fall_back(
        &self,
        reason: String,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionOutcome {
        let o = self.fallback.decide(id, name, input).await;
        let combined_reason = if o.reason.is_empty() {
            reason
        } else {
            format!("{reason} (fallback: {})", o.reason)
        };
        PermissionOutcome {
            decision: o.decision,
            reason: combined_reason,
            duration_ms: o.duration_ms,
        }
    }

    async fn apply_provider_error(
        &self,
        reason: String,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionOutcome {
        match self.on_provider_error {
            DegradedAction::Fallback => self.fall_back(reason, id, name, input).await,
            DegradedAction::Deny => PermissionOutcome::deny(reason),
        }
    }
}

#[async_trait]
impl PermissionHandler for AutoPermissionHandler {
    async fn decide(&self, id: &str, name: &str, input: &serde_json::Value) -> PermissionOutcome {
        if self.classifier.is_degraded() {
            warn!(tool = name, "auto classifier degraded");
            return self
                .fall_back("classifier degraded".into(), id, name, input)
                .await;
        }
        let (model, provider) = match self.resolver.resolve_for(TaskType::Classification) {
            Ok(p) => p,
            Err(e) => {
                warn!(tool = name, error = %e, "auto provider lookup failed");
                return self
                    .apply_provider_error(format!("provider lookup failed: {e}"), id, name, input)
                    .await;
            }
        };
        let context = self.context.recent().await;
        let cwd = self.context.cwd();
        let result = self
            .classifier
            .classify(name, input, &context, cwd, provider.as_ref(), &model)
            .await;
        info!(tool = name, decision = ?result.decision, reason = %result.reason, "auto-permission");
        PermissionOutcome {
            decision: result.decision,
            reason: result.reason,
            duration_ms: result.duration_ms,
        }
    }
}
