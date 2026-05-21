use std::sync::Arc;

use loopal_config::HarnessConfig;

use super::traits::{Governance, TurnHook};
use crate::agent_loop::degeneration_detector::DegenerationDetector;
use crate::agent_loop::diff_tracker::DiffTracker;
use crate::agent_loop::loop_detector::LoopDetector;
use crate::frontend::traits::AgentFrontend;

pub fn build_governance(harness: &HarnessConfig) -> Vec<Box<dyn Governance>> {
    vec![
        Box::new(LoopDetector::with_thresholds(
            harness.loop_warn_threshold,
            harness.loop_abort_threshold,
        )),
        Box::new(DegenerationDetector::new(
            harness.degeneration_barren_threshold,
            harness.degeneration_duplicate_text_threshold,
            harness.degeneration_wake_after_secs,
        )),
    ]
}

pub fn build_hooks(frontend: Arc<dyn AgentFrontend>) -> Vec<Box<dyn TurnHook>> {
    vec![Box::new(DiffTracker::new(frontend))]
}
