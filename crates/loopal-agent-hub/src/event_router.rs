//! Event routing — consumes raw agent events and forwards to the frontend (TUI).
//!
//! In Hub-only gateway mode, SubAgentSpawned no longer triggers TCP attach
//! (Hub manages all connections). Events are simply forwarded.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_protocol::AgentEvent;

use crate::hub::AgentHub;

/// Start the hub event loop. Consumes raw events and forwards to frontend.
pub fn start_event_loop(
    _hub: Arc<tokio::sync::Mutex<AgentHub>>,
    mut raw_rx: mpsc::Receiver<AgentEvent>,
    frontend_tx: mpsc::Sender<AgentEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("hub event loop started");
        while let Some(event) = raw_rx.recv().await {
            if frontend_tx.send(event).await.is_err() {
                tracing::info!("hub event loop: frontend closed");
                break;
            }
        }
        tracing::info!("hub event loop exited");
    })
}
