//! Monotonically increasing event ID generator + turn/correlation tracking.
//!
//! Used to assign unique IDs to `AgentEvent` instances for causality tracking.
//! IDs start at 1; 0 is reserved for "unset" (e.g. events from older producers).
//!
//! Turn ID and correlation ID are set by the runtime during execution and read
//! by event emitters to stamp outgoing events without changing the emit() trait.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

static NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(1);
static CURRENT_TURN_ID: AtomicU32 = AtomicU32::new(0);
static CURRENT_CORRELATION_ID: AtomicU64 = AtomicU64::new(0);

/// Generate the next unique event ID (monotonically increasing, never 0).
pub fn next_event_id() -> u64 {
    NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Set the current turn ID (called by runtime at turn boundaries).
pub fn set_current_turn_id(id: u32) {
    CURRENT_TURN_ID.store(id, Ordering::Relaxed);
}

/// Get the current turn ID (called by event emitters).
pub fn current_turn_id() -> u32 {
    CURRENT_TURN_ID.load(Ordering::Relaxed)
}

/// Set the current correlation ID (called by runtime for tool batches).
pub fn set_current_correlation_id(id: u64) {
    CURRENT_CORRELATION_ID.store(id, Ordering::Relaxed);
}

/// Get the current correlation ID (called by event emitters).
pub fn current_correlation_id() -> u64 {
    CURRENT_CORRELATION_ID.load(Ordering::Relaxed)
}
