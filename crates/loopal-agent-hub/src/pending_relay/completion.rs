use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Listening};
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::AgentEvent;
use tokio::sync::{Mutex, Notify, mpsc};
use tracing::warn;

use crate::Hub;

#[derive(Clone)]
pub(super) struct TerminalEventSink {
    event_tx: mpsc::Sender<AgentEvent>,
    ui_connections: Vec<Arc<Connection<Listening>>>,
    shutdown_signal: Arc<Notify>,
    redaction_seed: FinalSinkRedactionSeed,
}

impl TerminalEventSink {
    pub(super) fn from_hub(hub: &Hub) -> Self {
        Self {
            event_tx: hub.registry.event_sender(),
            ui_connections: hub
                .ui
                .clients
                .values()
                .map(|client| client.connection.clone())
                .collect(),
            shutdown_signal: hub.shutdown_signal.clone(),
            redaction_seed: hub.final_sink_redaction_seed(),
        }
    }
}

pub(crate) async fn deliver_terminal_event(
    hub: &Arc<Mutex<Hub>>,
    event: AgentEvent,
) -> Result<(), String> {
    let sink = {
        let hub = hub.lock().await;
        TerminalEventSink::from_hub(&hub)
    };
    match enqueue_terminal_event(&sink, event).await {
        Ok(()) => Ok(()),
        Err(message) => {
            invalidate_terminal_sink(sink).await;
            Err(message)
        }
    }
}

/// Finish an interaction outside the UI request task that claimed it.
///
/// UI IO loops abort their in-flight handlers on disconnect. Spawning the
/// irreversible response here ensures a handler cannot remove pending state
/// and then be cancelled before the agent receives its response.
pub(super) fn complete_detached(
    agent_conn: Arc<Connection<Listening>>,
    agent_ipc_id: i64,
    response: serde_json::Value,
    resolved_event: Option<(TerminalEventSink, AgentEvent)>,
) {
    tokio::spawn(async move {
        if let Some((sink, event)) = resolved_event
            && let Err(message) = enqueue_terminal_event(&sink, event).await
        {
            warn!(agent_ipc_id, %message);
            tokio::join!(
                close_after_terminal_failure(&agent_conn, agent_ipc_id),
                invalidate_terminal_sink(sink)
            );
            return;
        }
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            agent_conn.respond(agent_ipc_id, response),
        )
        .await;
        match result {
            Ok(Ok(())) => return,
            Ok(Err(error)) => {
                warn!(agent_ipc_id, %error, "interaction response failed; closing agent IPC");
            }
            Err(_) => {
                warn!(
                    agent_ipc_id,
                    "interaction response timed out; closing agent IPC"
                );
            }
        }
        close_after_terminal_failure(&agent_conn, agent_ipc_id).await;
    });
}

async fn enqueue_terminal_event(sink: &TerminalEventSink, event: AgentEvent) -> Result<(), String> {
    let event = sink.redaction_seed.guard_event(event);
    match tokio::time::timeout(Duration::from_secs(2), sink.event_tx.send(event)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err("interaction Resolved event queue closed".into()),
        Err(_) => Err("interaction Resolved event delivery timed out".into()),
    }
}

async fn invalidate_terminal_sink(sink: TerminalEventSink) {
    let ui_closers: Vec<_> = sink
        .ui_connections
        .into_iter()
        .map(|connection| {
            tokio::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(2), connection.close()).await;
            })
        })
        .collect();
    sink.shutdown_signal.notify_one();
    for closer in ui_closers {
        let _ = closer.await;
    }
}

async fn close_after_terminal_failure(agent_conn: &Connection<Listening>, agent_ipc_id: i64) {
    if tokio::time::timeout(Duration::from_secs(2), agent_conn.close())
        .await
        .is_err()
    {
        warn!(agent_ipc_id, "timed out closing unresponsive agent IPC");
    }
}
