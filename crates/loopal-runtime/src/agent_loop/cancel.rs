//! Per-turn cancellation scope.
//!
//! Bridges the cross-boundary `InterruptSignal` (set by TUI on ESC) into
//! a standard `CancellationToken` for structured async cancellation within
//! a single turn. All turn-scoped operations receive `&TurnCancel` instead
//! of raw `(&InterruptSignal, &Arc<Notify>)`.

use std::sync::Arc;

use loopal_protocol::InterruptSignal;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// Per-turn cancellation scope.
///
/// Created at the start of each turn in `run_loop`, dropped when the turn ends.
/// Encapsulates the bridge from `InterruptSignal` (TUI boundary) to
/// `CancellationToken` (runtime internal).
pub struct TurnCancel {
    token: CancellationToken,
    interrupt: InterruptSignal,
    interrupt_notify: Arc<Notify>,
}

impl TurnCancel {
    /// Create a new per-turn cancel scope.
    ///
    /// If the interrupt signal is already set (stale from a previous turn
    /// edge), the token is pre-cancelled so downstream checks see it
    /// immediately.
    pub fn new(interrupt: InterruptSignal, interrupt_notify: Arc<Notify>) -> Self {
        let token = CancellationToken::new();
        if interrupt.is_signaled() {
            token.cancel();
        }
        Self {
            token,
            interrupt,
            interrupt_notify,
        }
    }

    /// Check if cancellation has been requested (sync, non-blocking).
    ///
    /// Checks both the `CancellationToken` and the raw `InterruptSignal`.
    /// When a signal is detected but the token isn't cancelled yet, bridges
    /// the signal by cancelling the token — subsequent async operations see
    /// cancellation instantly via `cancelled()`.
    pub fn is_cancelled(&self) -> bool {
        if self.token.is_cancelled() {
            return true;
        }
        if self.interrupt.is_signaled() {
            self.token.cancel();
            true
        } else {
            false
        }
    }

    /// Wait for cancellation (async).
    ///
    /// Races `CancellationToken::cancelled()` against `Notify::notified()`.
    /// Returns **only** when cancellation is confirmed — spurious notify
    /// wakeups are handled internally by looping.
    pub async fn cancelled(&self) {
        loop {
            tokio::select! {
                biased;
                _ = self.token.cancelled() => return,
                _ = self.interrupt_notify.notified() => {
                    if self.interrupt.is_signaled() {
                        self.token.cancel();
                        return;
                    }
                    // Spurious wakeup — keep waiting
                }
            }
        }
    }
}
