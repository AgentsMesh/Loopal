//! Aggregates token usage counters across the agent session.
//!
//! Extracted from `AgentLoopRunner` to encapsulate the five token
//! accounting fields behind a single cohesive type.

/// Cumulative token usage across all LLM calls in a session.
#[derive(Debug, Clone, Default)]
pub struct TokenAccumulator {
    pub input: u32,
    pub output: u32,
    pub cache_creation: u32,
    pub cache_read: u32,
    pub thinking: u32,
}

impl TokenAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all counters to zero (e.g. on conversation clear).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
