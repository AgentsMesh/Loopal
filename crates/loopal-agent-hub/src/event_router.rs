//! Event routing — consumes raw agent events and broadcasts to subscribers.
//!
//! Three responsibilities, all driven by the same `raw_rx` consumer so
//! event ordering is preserved per agent:
//! 1. **UI broadcast** — emit each event to `UiDispatcher.event_broadcaster()`.
//!    UI clients (TUI / ACP / TCP attach) listen here for the live event
//!    stream and apply each event to their local `ViewClient` reducer.
//! 2. **Hub-side ViewState** — also apply each event to the originating
//!    agent's `ViewStateReducer` so `view/snapshot` returns the latest
//!    observable state. There is no separate `view/delta` channel; the
//!    UI broadcast is the incremental update stream.
//! 3. **Topology lifecycle** — project runtime state into topology nodes.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_view_state::ViewStateApplyOutcome;

use crate::hub::Hub;
use crate::topology::AgentLifecycle;

/// Start the hub event loop. Consumes raw events, applies them to the
/// per-agent ViewStateReducer (stamping the resulting `rev` onto the
/// event), and broadcasts to UI subscribers.
pub fn start_event_loop(
    hub: Arc<tokio::sync::Mutex<Hub>>,
    mut raw_rx: mpsc::Receiver<AgentEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("hub event loop started");
        let (broadcaster, redaction_seed) = {
            let h = hub.lock().await;
            (h.ui.event_broadcaster(), h.final_sink_redaction_seed())
        };
        while let Some(event) = raw_rx.recv().await {
            let mut event = redaction_seed.guard_event(event);
            match apply_to_view_state(&hub, &event).await {
                EventRoute::Broadcast(rev) => event.rev = rev,
                EventRoute::DropStale => {
                    tracing::debug!(agent = ?event.agent_name, "queued event belongs to a stale generation; dropping");
                    continue;
                }
                EventRoute::ResyncRequired => {
                    let h = hub.lock().await;
                    h.ui.request_resync();
                    tracing::warn!(agent = ?event.agent_name, "view projection revision gap; requesting snapshot resync");
                    continue;
                }
            }
            // Broadcast to all UI subscribers. Ignored error means no
            // active receivers — that's fine, ViewState is still updated.
            let _ = broadcaster.send(event);
        }
        tracing::info!("hub event loop exited");
    })
}

enum EventRoute {
    Broadcast(Option<u64>),
    DropStale,
    ResyncRequired,
}

/// Apply the event to the originating agent's `ViewStateReducer` so
/// `view/snapshot` reflects it. Returns the post-apply `rev` so the
/// caller can stamp it onto the broadcasted event copy. `None` when no
/// reducer was touched (cross-hub event, untargeted event, or
/// non-observable payload).
async fn apply_to_view_state(hub: &Arc<tokio::sync::Mutex<Hub>>, event: &AgentEvent) -> EventRoute {
    let Some(addr) = event.agent_name.as_ref() else {
        return EventRoute::Broadcast(None);
    };
    if !addr.is_local() {
        return EventRoute::Broadcast(None);
    }
    let reducer = {
        let mut h = hub.lock().await;
        if event
            .routing_generation
            .is_some_and(|generation| !h.registry.owns_generation(&addr.agent, generation))
        {
            return EventRoute::DropStale;
        }
        project_lifecycle(&mut h, &addr.agent, &event.payload);
        let Some(reducer) = h
            .registry
            .agent_view(&addr.agent)
            .or_else(|| h.remote_views.get(&addr.agent).cloned())
        else {
            return EventRoute::Broadcast(None);
        };
        reducer
    };
    let outcome = reducer.lock().await.apply_checked(event.payload.clone());
    if let Some(generation) = event.routing_generation {
        let h = hub.lock().await;
        if !h.registry.owns_generation(&addr.agent, generation) {
            return EventRoute::DropStale;
        }
    }
    match outcome {
        ViewStateApplyOutcome::Applied { revision } => EventRoute::Broadcast(Some(revision)),
        ViewStateApplyOutcome::NoOp => EventRoute::Broadcast(None),
        ViewStateApplyOutcome::ResyncRequired(_) => EventRoute::ResyncRequired,
    }
}

fn project_lifecycle(hub: &mut Hub, agent: &str, payload: &AgentEventPayload) {
    match payload {
        AgentEventPayload::Running | AgentEventPayload::Started => {
            hub.registry.set_lifecycle(agent, AgentLifecycle::Running);
        }
        AgentEventPayload::Error { message } => {
            hub.registry
                .set_lifecycle(agent, AgentLifecycle::Failed(message.clone()));
        }
        _ => {}
    }
}
