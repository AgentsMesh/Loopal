use std::time::Instant;

use loopal_provider_api::Provider;
use loopal_tool_api::PermissionDecision;
use tracing::{info, warn};

use crate::cache::ClassifierCache;
use crate::circuit_breaker::CircuitBreaker;
use crate::llm_call::{call_classifier, parse_response};
use crate::prompt;

pub struct ClassifierResult {
    pub decision: PermissionDecision,
    pub reason: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl ClassifierResult {
    pub fn ok(decision: PermissionDecision, reason: impl Into<String>) -> Self {
        Self {
            decision,
            reason: reason.into(),
            duration_ms: 0,
            error: None,
        }
    }

    pub fn error(reason: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            decision: PermissionDecision::Deny,
            reason: reason.into(),
            duration_ms: 0,
            error: Some(error.into()),
        }
    }
}

pub struct AutoClassifier {
    circuit_breaker: CircuitBreaker,
    cache: ClassifierCache,
    instructions: String,
}

impl AutoClassifier {
    pub fn new(instructions: String) -> Self {
        Self {
            circuit_breaker: CircuitBreaker::new(),
            cache: ClassifierCache::new(),
            instructions,
        }
    }

    pub fn new_with_thresholds(
        instructions: String,
        max_consecutive: u32,
        max_total: u32,
    ) -> Self {
        Self {
            circuit_breaker: CircuitBreaker::with_thresholds(max_consecutive, max_total),
            cache: ClassifierCache::new(),
            instructions,
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.circuit_breaker.is_degraded()
    }

    pub fn on_human_approval(&self, tool_name: &str) {
        self.circuit_breaker.record_approval(tool_name);
        self.circuit_breaker.reset_degradation();
    }

    #[doc(hidden)]
    pub fn force_degraded_for_test(&self, tool_name: &str) {
        for _ in 0..self.circuit_breaker.max_consecutive() {
            self.circuit_breaker.record_denial(tool_name);
        }
    }

    pub(crate) fn instructions(&self) -> &str {
        &self.instructions
    }

    pub(crate) fn breaker(&self) -> &CircuitBreaker {
        &self.circuit_breaker
    }

    pub async fn classify(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        recent_context: &str,
        cwd: &str,
        provider: &dyn Provider,
        model: &str,
    ) -> ClassifierResult {
        if let Some(cached) = self.cache.get(tool_name, input) {
            info!(tool = tool_name, decision = ?cached.decision, "auto-mode (cached)");
            return cached;
        }

        let start = Instant::now();
        let mut result = self
            .classify_uncached(tool_name, input, recent_context, cwd, provider, model)
            .await;
        result.duration_ms = start.elapsed().as_millis() as u64;
        self.cache.put(tool_name, input, &result);
        result
    }

    async fn classify_uncached(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        recent_context: &str,
        cwd: &str,
        provider: &dyn Provider,
        model: &str,
    ) -> ClassifierResult {
        let user_prompt = prompt::user_prompt(
            tool_name,
            input,
            &self.instructions,
            recent_context,
            cwd,
        );

        let result = call_classifier(provider, model, &user_prompt).await;

        match result {
            Ok(response) => match parse_response(&response) {
                Some((should_block, reason)) => {
                    let decision = if should_block {
                        self.circuit_breaker.record_denial(tool_name);
                        PermissionDecision::Deny
                    } else {
                        self.circuit_breaker.record_approval(tool_name);
                        PermissionDecision::Allow
                    };
                    info!(tool = tool_name, ?decision, reason = %reason, "auto-mode");
                    ClassifierResult::ok(decision, reason)
                }
                None => {
                    warn!(tool = tool_name, response = %response, "classifier parse failure");
                    self.circuit_breaker.record_error(tool_name);
                    ClassifierResult::error(
                        "Classifier response parse failure — blocking for safety",
                        format!("parse failure: '{response}'"),
                    )
                }
            },
            Err(e) => {
                warn!(tool = tool_name, error = %e, "classifier LLM error");
                self.circuit_breaker.record_error(tool_name);
                ClassifierResult::error(format!("Classifier error: {e}"), e.to_string())
            }
        }
    }
}
