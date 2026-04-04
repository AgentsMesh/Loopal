use std::collections::HashMap;
use std::sync::Mutex;

/// Maximum consecutive denials for a single tool before degrading to human.
const MAX_CONSECUTIVE_DENIALS: u32 = 3;
/// Maximum total denials in a session before degrading to human.
const MAX_TOTAL_DENIALS: u32 = 20;

/// Safety circuit breaker for Auto Mode.
///
/// Tracks denial counts and degrades to human approval when thresholds
/// are exceeded, preventing runaway classification loops.
pub struct CircuitBreaker {
    inner: Mutex<Inner>,
    max_consecutive: u32,
    max_total: u32,
}

struct Inner {
    /// Per-tool consecutive denial count.
    consecutive: HashMap<String, u32>,
    /// Session-wide total denial count.
    total_denials: u32,
    /// Whether the breaker has tripped.
    degraded: bool,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self::with_thresholds(MAX_CONSECUTIVE_DENIALS, MAX_TOTAL_DENIALS)
    }

    /// Create with custom thresholds (from HarnessConfig).
    pub fn with_thresholds(max_consecutive: u32, max_total: u32) -> Self {
        Self {
            inner: Mutex::new(Inner {
                consecutive: HashMap::new(),
                total_denials: 0,
                degraded: false,
            }),
            max_consecutive,
            max_total,
        }
    }

    /// Record a classifier denial. May trip the breaker.
    pub fn record_denial(&self, tool_name: &str) {
        let mut inner = self.inner.lock().unwrap();
        let count = inner.consecutive.entry(tool_name.to_string()).or_insert(0);
        *count += 1;
        let consecutive_exceeded = *count >= self.max_consecutive;
        inner.total_denials += 1;
        if consecutive_exceeded || inner.total_denials >= self.max_total {
            inner.degraded = true;
        }
    }

    /// Record a classifier approval. Resets the consecutive count for the tool.
    pub fn record_approval(&self, tool_name: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.consecutive.remove(tool_name);
    }

    /// Record a classifier error. Treated as a denial (fail-closed).
    pub fn record_error(&self, tool_name: &str) {
        self.record_denial(tool_name);
    }

    /// Whether the breaker has tripped (too many denials).
    pub fn is_degraded(&self) -> bool {
        self.inner.lock().unwrap().degraded
    }

    /// Reset degradation after a human approves a tool.
    pub fn reset_degradation(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.degraded = false;
        inner.consecutive.clear();
        inner.total_denials = 0;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}
