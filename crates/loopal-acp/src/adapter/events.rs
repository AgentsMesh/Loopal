//! ACP adapter: event loop and bootstrap drain.
//!
//! Dual-source select:
//! - `event_rx` (broadcast): structured AgentEvents from Hub
//! - `relay_rx` (mpsc): permission/question relay requests from Hub

use agent_client_protocol_schema::StopReason;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use tracing::warn;

use crate::adapter::AcpAdapter;
use crate::translate::{AcpNotification, translate_event};

impl AcpAdapter {
    /// Run the event loop during a session/prompt.
    pub(crate) async fn run_event_loop(&self, session_id: &str) -> StopReason {
        loop {
            let mut event_rx = self.event_rx.lock().await;
            let mut relay_rx = self.relay_rx.lock().await;
            tokio::select! {
                result = event_rx.recv() => {
                    drop(event_rx); drop(relay_rx);
                    match result {
                        Ok(event) => {
                            if let Some(r) = self.handle_event(&event, session_id).await {
                                return r;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            warn!("event broadcast closed");
                            return StopReason::EndTurn;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!(skipped = n, "event receiver lagged");
                        }
                    }
                }
                msg = relay_rx.recv() => {
                    drop(event_rx); drop(relay_rx);
                    let Some(msg) = msg else {
                        warn!("relay connection closed");
                        return StopReason::EndTurn;
                    };
                    if let Incoming::Request { id, method, params } = msg {
                        if method == methods::AGENT_PERMISSION.name {
                            self.handle_permission_request(id, params, session_id).await;
                        } else if method == methods::AGENT_QUESTION.name {
                            self.handle_question_request(id, params).await;
                        }
                    }
                }
            }
        }
    }

    /// Handle a structured AgentEvent from Hub broadcast.
    async fn handle_event(&self, event: &AgentEvent, session_id: &str) -> Option<StopReason> {
        match &event.payload {
            AgentEventPayload::AwaitingInput => return Some(StopReason::EndTurn),
            AgentEventPayload::MaxTurnsReached { .. } => {
                return Some(StopReason::MaxTurnRequests);
            }
            AgentEventPayload::Finished => return Some(StopReason::EndTurn),
            _ => {}
        }
        if let Some(notif) = translate_event(&event.payload, session_id) {
            match notif {
                AcpNotification::SessionUpdate(params) => {
                    self.acp_out.notify("session/update", params).await;
                }
                AcpNotification::Extension { method, params } => {
                    self.acp_out.notify(&method, params).await;
                }
            }
        }
        None
    }

    /// Drain bootstrap events until AwaitingInput or Finished.
    pub(crate) async fn drain_bootstrap_events(&self) {
        let mut rx = self.event_rx.lock().await;
        loop {
            match rx.recv().await {
                Ok(event)
                    if matches!(
                        event.payload,
                        AgentEventPayload::AwaitingInput | AgentEventPayload::Finished
                    ) =>
                {
                    return;
                }
                Err(_) => return,
                _ => continue,
            }
        }
    }
}
