//! Event routing — consumes raw agent events and broadcasts to subscribers.
//!
//! Two responsibilities, both driven by the same `raw_rx` consumer so
//! event ordering is preserved per agent:
//! 1. **UI broadcast** — emit each event to `UiDispatcher.event_broadcaster()`.
//!    UI clients (TUI / ACP / TCP attach) listen here for the live event
//!    stream and apply each event to their local `ViewClient` reducer.
//! 2. **Hub-side ViewState** — also apply each event to the originating
//!    agent's `ViewStateReducer` so `view/snapshot` returns the latest
//!    observable state. There is no separate `view/delta` channel; the
//!    UI broadcast is the incremental update stream.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_protocol::AgentEvent;

use crate::hub::Hub;

/// Start the hub event loop. Consumes raw events, applies them to the
/// per-agent ViewStateReducer (stamping the resulting `rev` onto the
/// event), and broadcasts to UI subscribers.
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
        while let Some(mut event) = raw_rx.recv().await {
            event.rev = apply_to_view_state(&hub, &event).await;
            // Broadcast to all UI subscribers. Ignored error means no
            // active receivers — that's fine, ViewState is still updated.
            let _ = broadcaster.send(event);
        }
        tracing::info!("hub event loop exited");
    })
}

/// Apply the event to the originating agent's `ViewStateReducer` so
/// `view/snapshot` reflects it. Returns the post-apply `rev` so the
/// caller can stamp it onto the broadcasted event copy. `None` when no
/// reducer was touched (cross-hub event, untargeted event, or
/// non-observable payload).
async fn apply_to_view_state(
    hub: &Arc<tokio::sync::Mutex<Hub>>,
    event: &AgentEvent,
) -> Option<u64> {
    let addr = event.agent_name.as_ref()?;
    if !addr.is_local() {
        return None;
    }
    let reducer = {
        let h = hub.lock().await;
        h.registry
            .agents
            .get(&addr.agent)
            .map(|ma| ma.view.clone())?
    };
    let mut guard = reducer.lock().await;
    guard.apply(event.payload.clone())
}
