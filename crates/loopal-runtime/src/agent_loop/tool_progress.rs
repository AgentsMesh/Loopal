//! Periodic progress reporter for long-running tools (e.g. Bash).
//!
//! Spawns a background task that emits `ToolProgress` events at regular
//! intervals until the tool completes and the handle is aborted.

use std::sync::Arc;
use std::time::{Duration, Instant};

use loopal_protocol::AgentEventPayload;
use loopal_tool_api::{OutputTail, TimeoutSecs};
use tokio::task::JoinHandle;

use crate::frontend::traits::EventEmitter;

/// Minimum timeout (seconds) to activate progress reporting.
const PROGRESS_THRESHOLD_SECS: u64 = 10;

/// Interval between progress reports.
const REPORT_INTERVAL: Duration = Duration::from_secs(2);

/// Spawn a progress reporter if the tool warrants it.
///
/// Returns `Some(handle)` that must be `.abort()`-ed when the tool finishes.
/// Returns `None` if the tool doesn't need progress reporting.
pub fn maybe_spawn_progress(
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_id: String,
    emitter: Box<dyn EventEmitter>,
    tail: Option<Arc<OutputTail>>,
) -> Option<JoinHandle<()>> {
    if tool_name != "Bash" {
        return None;
    }
    let timeout = TimeoutSecs::from_tool_input(tool_input, 300);
    if timeout.as_secs() < PROGRESS_THRESHOLD_SECS {
        return None;
    }

    let name = tool_name.to_string();
    Some(tokio::spawn(async move {
        let start = Instant::now();
        let mut interval = tokio::time::interval(REPORT_INTERVAL);
        interval.tick().await; // skip first immediate tick
        loop {
            interval.tick().await;
            let elapsed_ms = start.elapsed().as_millis() as u64;
            // output_tail carries only real stdout content (time info is
            // rendered independently by the TUI from started_at/tool_input).
            let output_tail = match &tail {
                Some(t) => t.snapshot(),
                None => String::new(),
            };
            let _ = emitter
                .emit(AgentEventPayload::ToolProgress {
                    id: tool_id.clone(),
                    name: name.clone(),
                    output_tail,
                    elapsed_ms,
                })
                .await;
        }
    }))
}
