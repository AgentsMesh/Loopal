//! Per-turn cancellation scope.
//!
//! Bridges the cross-boundary `InterruptSignal` (set by any consumer) into
//! a standard `CancellationToken` for structured async cancellation within
//! a single turn. All turn-scoped operations receive `&TurnCancel` instead
//! of raw `(&InterruptSignal, &Arc<watch::Sender<u64>>)`.

use std::sync::Arc;

use loopal_protocol::InterruptSignal;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Per-turn cancellation scope.
///
/// Created at the start of each turn in `run_loop`, dropped when the turn ends.
/// Encapsulates the bridge from `InterruptSignal` (consumer boundary) to
/// `CancellationToken` (runtime internal).
///
/// Uses `watch::Receiver` for async wakeup — level-triggered, so signals
/// are never lost even if no waiter is active at the moment of signaling.
pub struct TurnCancel {
    token: CancellationToken,
    interrupt: InterruptSignal,
    interrupt_rx: watch::Receiver<u64>,
    /// Hold a reference to the sender to keep the watch channel alive.
    _interrupt_tx: Arc<watch::Sender<u64>>,
}

impl TurnCancel {
    /// Create a new per-turn cancel scope.
    ///
    /// If the interrupt signal is already set (stale from a previous turn
    /// edge), the token is pre-cancelled so downstream checks see it
    /// immediately.
    pub fn new(interrupt: InterruptSignal, interrupt_tx: Arc<watch::Sender<u64>>) -> Self {
        let token = CancellationToken::new();
        let interrupt_rx = interrupt_tx.subscribe();
        if interrupt.is_signaled() {
            tracing::debug!("TurnCancel: pre-cancelled due to stale interrupt");
            token.cancel();
        }
        Self {
            token,
            interrupt,
            interrupt_rx,
            _interrupt_tx: interrupt_tx,
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
    /// First performs an eager sync check of `InterruptSignal` to catch
    /// stale signals immediately. Then races `CancellationToken::cancelled()`
    /// against `watch::Receiver::changed()`.
    ///
    /// `watch::Receiver::changed()` is level-triggered: it returns immediately
    /// if the value has changed since the receiver was created (or last observed).
    /// This eliminates the signal-loss bug inherent in `Notify::notify_waiters()`.
    pub async fn cancelled(&self) {
        // Eager sync check — catches signals set before watch saw them
        if self.interrupt.is_signaled() {
            self.token.cancel();
            return;
        }
        let mut rx = self.interrupt_rx.clone();
        tokio::select! {
            biased;
            _ = self.token.cancelled() => {}
            result = rx.changed() => {
                // Ok: sender called send_modify (interrupt signaled).
                // Err: sender dropped (system shutting down) — return to let
                // the caller's select! pick this branch and exit gracefully.
                drop(result);
                if self.interrupt.is_signaled() {
                    self.token.cancel();
                }
            }
        }
    }
}
