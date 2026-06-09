use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use loopal_decision_api::DecisionMode;

/// Runtime-mutable decision mode shared between the classifier handlers and
/// the agent loop. The classifier handlers read it on every request; the
/// `DecisionModeSwitch` control command writes it. `Manual` short-circuits
/// the classifier and delegates straight to the IPC fallback.
#[derive(Clone)]
pub struct DecisionCell(Arc<AtomicU8>);

impl DecisionCell {
    pub fn new(mode: DecisionMode) -> Self {
        Self(Arc::new(AtomicU8::new(mode.as_u8())))
    }

    pub fn get(&self) -> DecisionMode {
        DecisionMode::from_u8(self.0.load(Ordering::Relaxed))
    }

    pub fn set(&self, mode: DecisionMode) {
        self.0.store(mode.as_u8(), Ordering::Relaxed);
    }
}
