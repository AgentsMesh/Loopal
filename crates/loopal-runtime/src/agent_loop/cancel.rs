//! Per-turn cancellation scope.
//!
//! Bridges the cross-boundary `InterruptSignal` (set by any consumer) into
//! a standard `CancellationToken` for structured async cancellation within
//! a single turn. All turn-scoped operations receive `&TurnCancel` instead
//! of raw `(&InterruptSignal, &Arc<watch::Sender<u64>>)`.

use std::sync::{Arc, Mutex};

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
    /// Keeps `token()` live for callees in other crates. Without this bridge,
    /// only callers polling `TurnCancel::cancelled()` observed interrupts, so
    /// compaction waiting on the exported token could not be cancelled.
    bridge_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
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
        let bridge_task = tokio::runtime::Handle::try_current().ok().map(|runtime| {
            let token = token.clone();
            let interrupt = interrupt.clone();
            let mut interrupt_rx = interrupt_tx.subscribe();
            runtime.spawn(async move {
                if interrupt.is_signaled() {
                    token.cancel();
                    return;
                }
                while interrupt_rx.changed().await.is_ok() {
                    if interrupt.is_signaled() {
                        token.cancel();
                        return;
                    }
                }
            })
        });
        Self {
            token,
            interrupt,
            interrupt_rx,
            bridge_task: Mutex::new(bridge_task),
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

    /// Borrow the underlying token. Lets callers in other crates (e.g.
    /// `loopal-context`) wire `select!` directly against the same
    /// cancellation source without re-bridging InterruptSignal.
    pub fn token(&self) -> &CancellationToken {
        if !self.is_cancelled() {
            let mut bridge_task = self
                .bridge_task
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if bridge_task.is_none() {
                let runtime = tokio::runtime::Handle::try_current()
                    .expect("TurnCancel::token() requires a Tokio runtime to bridge interrupts");
                let token = self.token.clone();
                let interrupt = self.interrupt.clone();
                let mut interrupt_rx = self._interrupt_tx.subscribe();
                *bridge_task = Some(runtime.spawn(async move {
                    if interrupt.is_signaled() {
                        token.cancel();
                        return;
                    }
                    while interrupt_rx.changed().await.is_ok() {
                        if interrupt.is_signaled() {
                            token.cancel();
                            return;
                        }
                    }
                }));
            }
        }
        &self.token
    }
}

impl Drop for TurnCancel {
    fn drop(&mut self) {
        let bridge_task = self
            .bridge_task
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(bridge_task) = bridge_task {
            bridge_task.abort();
        }
    }
}
