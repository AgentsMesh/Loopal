//! Shared interrupt signal for cancelling in-progress agent work.
//!
//! Used by both cancel-to-interrupt and send-message-while-busy flows.
//! The consumer calls `signal()`, the runtime polls `is_signaled()` at
//! key checkpoints (per stream chunk, before tool execution).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A cheaply cloneable interrupt signal backed by `AtomicBool`.
///
/// Both the consumer and the runtime hold clones of the same instance.
#[derive(Clone, Debug)]
pub struct InterruptSignal(Arc<AtomicBool>);

impl InterruptSignal {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Raise the interrupt flag (called by consumer on cancel or new message).
    pub fn signal(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Check whether the interrupt flag is raised (non-consuming read).
    pub fn is_signaled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Atomically check and clear the flag. Returns `true` if was signaled.
    ///
    /// Uses `compare_exchange` to avoid a race where a second `signal()`
    /// between a plain `is_signaled()` + `reset()` would be lost.
    pub fn take(&self) -> bool {
        self.0
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}

impl Default for InterruptSignal {
    fn default() -> Self {
        Self::new()
    }
}
