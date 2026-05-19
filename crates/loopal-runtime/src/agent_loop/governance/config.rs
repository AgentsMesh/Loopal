use std::sync::Arc;

use loopal_config::HarnessConfig;

use super::traits::{Governance, TurnHook};
use crate::agent_loop::diff_tracker::DiffTracker;
use crate::agent_loop::loop_detector::LoopDetector;
use crate::frontend::traits::AgentFrontend;

pub fn build_governance(harness: &HarnessConfig) -> Vec<Box<dyn Governance>> {
    vec![Box::new(LoopDetector::with_thresholds(
        harness.loop_warn_threshold,
        harness.loop_abort_threshold,
    ))]
}

pub fn build_hooks(frontend: Arc<dyn AgentFrontend>) -> Vec<Box<dyn TurnHook>> {
    vec![Box::new(DiffTracker::new(frontend))]
}
