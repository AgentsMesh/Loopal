use std::collections::HashMap;
use std::sync::Mutex;

const MAX_CONSECUTIVE_DENIALS: u32 = 3;
const MAX_TOTAL_DENIALS: u32 = 20;

pub struct CircuitBreaker {
    inner: Mutex<Inner>,
    max_consecutive: u32,
    max_total: u32,
}

struct Inner {
    consecutive: HashMap<String, u32>,
    total_denials: u32,
    degraded: bool,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self::with_thresholds(MAX_CONSECUTIVE_DENIALS, MAX_TOTAL_DENIALS)
    }

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

    pub fn record_approval(&self, tool_name: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.consecutive.remove(tool_name);
    }

    pub fn record_error(&self, tool_name: &str) {
        self.record_denial(tool_name);
    }

    pub fn is_degraded(&self) -> bool {
        self.inner.lock().unwrap().degraded
    }

    pub fn reset_degradation(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.degraded = false;
        inner.consecutive.clear();
        inner.total_denials = 0;
    }

    #[doc(hidden)]
    pub fn max_consecutive(&self) -> u32 {
        self.max_consecutive
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}
