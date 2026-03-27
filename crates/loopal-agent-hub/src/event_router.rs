//! Event routing — consumes raw agent events, handles SubAgentSpawned
//! auto-attach in background, then forwards all events to the frontend.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::hub::AgentHub;

const ATTACH_MAX_RETRIES: u32 = 3;
const ATTACH_RETRY_DELAY_MS: u64 = 500;

/// Start the hub event loop. Consumes raw events from the bridge,
/// spawns background attach on SubAgentSpawned, and forwards to frontend.
///
/// Attach runs in a separate background task with retry. Each attempt
/// acquires and releases the hub lock independently, so the hub is never
/// blocked during network I/O or retry delays.
pub fn start_event_loop(
    hub: Arc<tokio::sync::Mutex<AgentHub>>,
    mut raw_rx: mpsc::Receiver<AgentEvent>,
    frontend_tx: mpsc::Sender<AgentEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = raw_rx.recv().await {
            if let AgentEventPayload::SubAgentSpawned {
                ref name,
                pid,
                port,
                ref token,
            } = event.payload
            {
                let hub_clone = hub.clone();
                let name = name.clone();
                let token = token.clone();
                tokio::spawn(attach_with_retry(hub_clone, name, pid, port, token));
            }
            if frontend_tx.send(event).await.is_err() {
                break;
            }
        }
        // Event source closed or frontend gone — clean up sub-agents.
        // Primary connections are managed by bootstrap/process lifecycle.
        let mut h = hub.lock().await;
        h.detach_all();
        tracing::debug!("hub event loop exited, detached all sub-agents");
    })
}

/// Attach to a sub-agent with retry. Each attempt independently locks
/// the hub — the lock is NOT held across retries or sleep delays.
async fn attach_with_retry(
    hub: Arc<tokio::sync::Mutex<AgentHub>>,
    name: String,
    _pid: u32,
    port: u16,
    token: String,
) {
    for attempt in 0..ATTACH_MAX_RETRIES {
        let result = {
            let mut h = hub.lock().await;
            h.attach(&name, port, &token).await
            // lock released here
        };
        match result {
            Ok(()) => return,
            Err(e) => {
                if attempt + 1 < ATTACH_MAX_RETRIES {
                    let delay = ATTACH_RETRY_DELAY_MS * (attempt as u64 + 1);
                    tracing::debug!(
                        agent = %name, attempt = attempt + 1, error = %e,
                        delay_ms = delay, "attach failed, retrying",
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                } else {
                    tracing::warn!(
                        agent = %name, error = %e,
                        "failed to auto-attach after {ATTACH_MAX_RETRIES} attempts",
                    );
                }
            }
        }
    }
}
