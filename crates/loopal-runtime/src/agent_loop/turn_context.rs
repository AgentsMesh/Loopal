use std::collections::BTreeSet;
use std::time::Instant;

use loopal_provider_api::ContinuationIntent;

use super::cancel::TurnCancel;
use super::turn_metrics::TurnMetrics;

pub struct TurnContext {
    pub turn_id: u32,
    pub cancel: TurnCancel,
    pub started_at: Instant,
    pub modified_files: BTreeSet<String>,
    pub pending_warnings: Vec<String>,
    pub pending_continuation: Option<ContinuationIntent>,
    pub metrics: TurnMetrics,
}

impl TurnContext {
    pub fn new(turn_id: u32, cancel: TurnCancel) -> Self {
        Self {
            turn_id,
            cancel,
            started_at: Instant::now(),
            modified_files: BTreeSet::new(),
            pending_warnings: Vec::new(),
            pending_continuation: None,
            metrics: TurnMetrics::default(),
        }
    }
}
