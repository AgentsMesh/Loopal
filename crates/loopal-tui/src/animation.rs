//! Process-local clock for purely visual animation.
//!
//! View-state elapsed timers describe business lifecycles. They may be
//! re-anchored or absent after a snapshot crosses the Hub/TUI boundary, so
//! renderers must not use them to choose animation frames.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Monotonic elapsed time shared by all animations in the TUI process.
pub(crate) fn elapsed() -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

/// Select a deterministic spinner frame from a sampled animation clock.
pub(crate) fn spinner_frame(elapsed: Duration) -> &'static str {
    let idx = (elapsed.as_millis() / 100) as usize % SPINNER.len();
    SPINNER[idx]
}
