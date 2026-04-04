use std::time::Instant;

use loopal_provider_api::Provider;
use loopal_tool_api::PermissionDecision;
use tracing::{info, warn};

use crate::cache::ClassifierCache;
use crate::circuit_breaker::CircuitBreaker;
use crate::llm_call::{call_classifier, parse_response};
use crate::prompt;

/// Result of a classifier decision.
pub struct ClassifierResult {
    pub decision: PermissionDecision,
    pub reason: String,
    /// Classification wall-clock time in milliseconds (0 for cached results).
    pub duration_ms: u64,
}

/// LLM-based permission classifier for Auto Mode.
///
/// Calls a lightweight LLM to decide whether a tool call should be
/// allowed or blocked. Uses a circuit breaker to degrade to human
/// approval on repeated denials or errors.
pub struct AutoClassifier {
    circuit_breaker: CircuitBreaker,
    cache: ClassifierCache,
    /// Project instructions (LOOPAL.md content) for classifier context.
    instructions: String,
    /// Project working directory for path-based safety decisions.
    cwd: String,
}

impl AutoClassifier {
    pub fn new(instructions: String, cwd: String) -> Self {
        Self {
            circuit_breaker: CircuitBreaker::new(),
            cache: ClassifierCache::new(),
            instructions,
            cwd,
        }
    }

    /// Create with custom circuit breaker thresholds (from HarnessConfig).
    pub fn new_with_thresholds(
        instructions: String,
        cwd: String,
        max_consecutive: u32,
        max_total: u32,
    ) -> Self {
        Self {
            circuit_breaker: CircuitBreaker::with_thresholds(max_consecutive, max_total),
            cache: ClassifierCache::new(),
            instructions,
            cwd,
        }
    }

    /// Whether the circuit breaker has tripped (too many denials/errors).
    pub fn is_degraded(&self) -> bool {
        self.circuit_breaker.is_degraded()
    }

    /// Reset degradation after human approves a tool in degraded mode.
    pub fn on_human_approval(&self, tool_name: &str) {
        self.circuit_breaker.record_approval(tool_name);
        self.circuit_breaker.reset_degradation();
    }

    /// Classify a tool call as Allow or Deny via LLM.
    ///
    /// Returns a cached result if available; otherwise calls the LLM
    /// and caches the result for future identical calls.
    pub async fn classify(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        recent_context: &str,
        provider: &dyn Provider,
        model: &str,
    ) -> ClassifierResult {
        // Cache hit — skip LLM call entirely.
        if let Some(cached) = self.cache.get(tool_name, input) {
            info!(tool = tool_name, decision = ?cached.decision, "auto-mode (cached)");
            return cached;
        }

        let start = Instant::now();
        let mut result = self
            .classify_uncached(tool_name, input, recent_context, provider, model)
            .await;
        result.duration_ms = start.elapsed().as_millis() as u64;
        self.cache.put(tool_name, input, &result);
        result
    }

    /// Core classification logic — always calls the LLM.
    async fn classify_uncached(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        recent_context: &str,
        provider: &dyn Provider,
        model: &str,
    ) -> ClassifierResult {
        let user_prompt = prompt::user_prompt(
            tool_name,
            input,
            &self.instructions,
            recent_context,
            &self.cwd,
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
                    ClassifierResult {
                        decision,
                        reason,
                        duration_ms: 0,
                    }
                }
                None => {
                    warn!(tool = tool_name, response = %response, "classifier parse failure");
                    self.circuit_breaker.record_error(tool_name);
                    ClassifierResult {
                        decision: PermissionDecision::Deny,
                        reason: "Classifier response parse failure — blocking for safety".into(),
                        duration_ms: 0,
                    }
                }
            },
            Err(e) => {
                warn!(tool = tool_name, error = %e, "classifier LLM error");
                self.circuit_breaker.record_error(tool_name);
                ClassifierResult {
                    decision: PermissionDecision::Deny,
                    reason: format!("Classifier error: {e}"),
                    duration_ms: 0,
                }
            }
        }
    }
}
