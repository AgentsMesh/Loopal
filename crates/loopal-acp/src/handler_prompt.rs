//! Prompt handling — ACP `session/prompt` event loop.

use serde_json::Value;
use tracing::warn;

use loopal_protocol::{AgentEventPayload, Envelope, MessageSource};
use loopal_runtime::AgentInput;

use crate::handler::AcpHandler;
use crate::translate::translate_event;
use crate::types::*;

impl AcpHandler {
    pub(crate) async fn handle_prompt(&self, id: i64, params: Value) {
        let params: PromptParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                self.transport
                    .respond_error(id, crate::jsonrpc::INVALID_REQUEST, &e.to_string())
                    .await;
                return;
            }
        };

        let guard = self.session.lock().await;
        let session = match guard.as_ref() {
            Some(s) if s.id == params.session_id => s,
            _ => {
                self.transport
                    .respond_error(id, crate::jsonrpc::INVALID_REQUEST, "session not found")
                    .await;
                return;
            }
        };

        // Extract text from ACP content blocks
        let text: String = params
            .prompt
            .iter()
            .map(|block| match block {
                AcpContentBlock::Text { text } => text.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Drain any bootstrap events (Started, AwaitingInput) left in the
        // channel from session/new.  At this point the agent loop is blocked
        // on recv_input(), so no real processing events can be present — only
        // the initial lifecycle events that were emitted before any prompt.
        {
            let mut rx = session.event_rx.lock().await;
            while rx.try_recv().is_ok() {}
        }

        // Forward as Envelope to the agent loop
        let envelope = Envelope::new(MessageSource::Human, "main", text);
        if session
            .input_tx
            .send(AgentInput::Message(envelope))
            .await
            .is_err()
        {
            self.transport
                .respond_error(
                    id,
                    crate::jsonrpc::INTERNAL_ERROR,
                    "agent loop disconnected",
                )
                .await;
            return;
        }

        // Event loop: translate AgentEvents → ACP session/update notifications
        let stop_reason = self.run_event_loop(session).await;

        let result = PromptResult { stop_reason };
        let value = serde_json::to_value(result).unwrap_or_default();
        self.transport.respond(id, value).await;
    }

    async fn run_event_loop(&self, session: &crate::handler::ActiveSession) -> StopReason {
        let mut rx = session.event_rx.lock().await;
        loop {
            match rx.recv().await {
                Some(event) => {
                    match &event.payload {
                        AgentEventPayload::AwaitingInput => {
                            return StopReason::EndTurn;
                        }
                        AgentEventPayload::MaxTurnsReached { .. } => {
                            return StopReason::MaxTurnRequests;
                        }
                        AgentEventPayload::Finished => {
                            return StopReason::EndTurn;
                        }
                        _ => {}
                    }

                    // Translate and emit as ACP notification
                    if let Some(params) = translate_event(&event.payload, &session.id) {
                        self.transport.notify("session/update", params).await;
                    }
                }
                None => {
                    warn!("agent event channel closed unexpectedly");
                    return StopReason::EndTurn;
                }
            }
        }
    }
}
