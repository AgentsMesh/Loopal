//! Event routing — consumes raw agent events and broadcasts to all subscribers.
//!
//! Uses broadcast channel for multi-consumer delivery. Each client (TUI, ACP)
//! subscribes independently via `hub.subscribe_events()`.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_protocol::AgentEvent;

use crate::hub::Hub;

/// Start the hub event loop. Consumes raw events and broadcasts to all subscribers.
pub fn start_event_loop(
    hub: Arc<tokio::sync::Mutex<Hub>>,
    mut raw_rx: mpsc::Receiver<AgentEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("hub event loop started");
        let broadcaster = {
            let h = hub.lock().await;
            h.ui.event_broadcaster()
        };
        while let Some(event) = raw_rx.recv().await {
            // Broadcast to all subscribers. Ignoring error means no active receivers.
            let _ = broadcaster.send(event);
        }
        tracing::info!("hub event loop exited");
    })
}
