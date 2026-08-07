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
    /// Latest caller-visible text produced during this turn. This lives on the
    /// turn context, rather than the happy-path return value, so cancellation
    /// and provider errors can still return useful partial work.
    best_effort_output: String,
    // 唯一 setter: handle_request_idle (turn_exec::ToolResultsWritten 分支 take).
    tool_signaled_turn_end: bool,
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
            best_effort_output: String::new(),
            tool_signaled_turn_end: false,
        }
    }

    pub(super) fn record_output(&mut self, text: &str) {
        self.best_effort_output.clear();
        self.best_effort_output.push_str(text);
    }

    pub(super) fn best_effort_output(&self) -> &str {
        &self.best_effort_output
    }

    pub(super) fn signal_turn_end_after_tools(&mut self) {
        self.tool_signaled_turn_end = true;
    }

    pub fn turn_end_after_tools_signaled(&self) -> bool {
        self.tool_signaled_turn_end
    }

    pub(super) fn take_turn_end_signal(&mut self) -> bool {
        std::mem::take(&mut self.tool_signaled_turn_end)
    }
}
