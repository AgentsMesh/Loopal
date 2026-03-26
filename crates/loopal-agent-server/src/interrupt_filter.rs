//! Interrupt notification filter for IPC incoming message stream.
//!
//! Sits between the Connection's `incoming_rx` and `IpcFrontend`, intercepting
//! `agent/interrupt` notifications so they are processed immediately — even
//! while the agent loop is busy executing tools (when `recv_input()` is not
//! running). Non-interrupt messages are forwarded unchanged.

use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::InterruptSignal;

/// Spawn a background task that filters `agent/interrupt` notifications out of
/// the incoming message stream, setting `interrupt` + waking `interrupt_tx`.
///
/// Returns a new `Receiver<Incoming>` that contains everything **except**
/// interrupt notifications. Callers pass this filtered receiver to
/// `IpcFrontend::new()` instead of the raw one from `Connection::start()`.
pub fn spawn(
    mut incoming_rx: mpsc::Receiver<Incoming>,
    interrupt: InterruptSignal,
    interrupt_tx: Arc<watch::Sender<u64>>,
) -> mpsc::Receiver<Incoming> {
    let (tx, rx) = mpsc::channel(256);
    tokio::spawn(async move {
        while let Some(msg) = incoming_rx.recv().await {
            if let Incoming::Notification { ref method, .. } = msg {
                if method == methods::AGENT_INTERRUPT.name {
                    interrupt.signal();
                    interrupt_tx.send_modify(|v| *v = v.wrapping_add(1));
                    continue;
                }
            }
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });
    rx
}
