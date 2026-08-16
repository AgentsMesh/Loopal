use std::collections::HashSet;

use super::tool_result_sink::PendingToolResult;

pub struct StreamingToolHandle;

impl StreamingToolHandle {
    pub fn empty() -> Self {
        Self
    }

    pub fn early_started_ids(&self) -> HashSet<String> {
        HashSet::new()
    }

    pub(super) async fn take_results(self) -> Vec<(usize, PendingToolResult)> {
        Vec::new()
    }

    pub fn discard(self) {}

    pub fn has_early_tools(&self) -> bool {
        false
    }
}

impl Default for StreamingToolHandle {
    fn default() -> Self {
        Self::empty()
    }
}
