use loopal_protocol::{AgentStatus, Envelope};
use tracing::error;

use super::input::WaitResult;
use super::runner::AgentLoopRunner;
use super::turn_trigger_map::envelope_to_trigger;

impl AgentLoopRunner {
    pub(super) async fn ingest_message(&mut self, env: &Envelope) -> WaitResult {
        let was_suspended = matches!(self.status, AgentStatus::Suspended);
        if was_suspended
            && env.source.wakes_suspended_session()
            && let Err(err) = self.transition(AgentStatus::Running).await
        {
            tracing::warn!(error = %err, "transition out of Suspended on human input failed");
        }
        let was_closed = !self.continuation_gate.is_open();
        // Automatic/peer envelopes may already be queued when Suspend wins the
        // turn-boundary race. They must not reopen the continuation gate. A
        // human envelope may do so only after the Running transition succeeds.
        let may_open_gate = !matches!(self.status, AgentStatus::Suspended);
        if may_open_gate {
            self.continuation_gate.open_for_envelope();
        }
        if was_closed && may_open_gate {
            let summary = self.continuation_gate.summary();
            if let Err(err) = self
                .emit(loopal_protocol::AgentEventPayload::ContinuationGateChanged(
                    summary,
                ))
                .await
            {
                tracing::warn!(error = %err, "ContinuationGateChanged emit failed on ingest");
            }
        }
        if self.turns.current_turn_id().is_some() {
            self.finalize_turn_cancellation(loopal_turn::CancelledCause::ParentTurnAborted)
                .await;
        }
        let Some(_turn_id) = self.start_turn_record(envelope_to_trigger(env)) else {
            error!(
                envelope_id = %env.id,
                "TurnStarted persist failed; dropping envelope to avoid orphan message on disk"
            );
            if let Err(emit_err) = self
                .emit(loopal_protocol::AgentEventPayload::Error {
                    message: format!(
                        "Failed to start turn for envelope {}: persist log unavailable",
                        env.id
                    ),
                })
                .await
            {
                tracing::warn!(error = %emit_err, "Error event emit failed after ingest abort");
            }
            return WaitResult::MessageAdded;
        };
        let ephemeral = env.source.is_ephemeral_in_history();
        if !ephemeral && self.params.session.title.is_empty() {
            let title = extract_title(&env.content.text);
            if !title.is_empty() {
                self.params.session.title = title;
                if let Err(e) = self
                    .params
                    .deps
                    .session_manager
                    .update_session(&self.params.session)
                {
                    error!(error = %e, "failed to persist session title");
                }
            }
        }

        let message_id = env.id.to_string();
        // reason: emit before tracking the id — a failed emit must not leave
        // an orphan InboxConsumed without its enqueued counterpart.
        match self
            .emit(loopal_protocol::AgentEventPayload::InboxEnqueued {
                envelope_id: message_id.clone(),
                source: env.source.clone(),
                content: env.content.text.clone(),
                summary: env.summary.clone(),
            })
            .await
        {
            Ok(()) => self.pending_consumed_ids.push(message_id),
            Err(e) => tracing::warn!(
                error = %e,
                "InboxEnqueued emit failed; LLM will see the message but UI/observers won't"
            ),
        }

        self.notify_observers_envelope_received(&env.source);

        if matches!(env.source, loopal_protocol::MessageSource::Human)
            && let Some(handler) = self.params.workflow_input_handler.clone()
        {
            let recent_context = recent_context(self.turns.view().messages());
            match handler.handle(env, &recent_context).await {
                Ok(loopal_runtime_disposition::WorkflowInputDisposition::Handled) => {
                    self.end_turn_record(loopal_turn::TurnOutcome::Complete);
                    return WaitResult::WorkflowHandled;
                }
                Ok(loopal_runtime_disposition::WorkflowInputDisposition::Direct) => {}
                Err(error) => {
                    tracing::error!(error = %error, "workflow input handler failed");
                    let message = format!("workflow input handler failed: {error}");
                    if let Err(emit_error) = self.transition_error(message.clone()).await {
                        tracing::warn!(
                            error = %emit_error,
                            handler_error = %error,
                            "failed to emit workflow input handler error"
                        );
                    }
                    return WaitResult::WorkflowFailed(message);
                }
            }
        }

        WaitResult::MessageAdded
    }
}

// Keep context provider-neutral and bounded. The planner receives only the
// visible text projection, never tool metadata or transport authority.
fn recent_context(messages: &[loopal_provider_api::Message]) -> String {
    const MAX_MESSAGES: usize = 16;
    const MAX_BYTES: usize = 8 * 1_024;
    let mut out = String::new();
    for message in messages.iter().rev().take(MAX_MESSAGES).rev() {
        let role = match message.role {
            loopal_provider_api::MessageRole::User => "user",
            loopal_provider_api::MessageRole::Assistant => "assistant",
            loopal_provider_api::MessageRole::System => "system",
        };
        let line = format!("{role}: {}\n", message.text_content());
        if out.len().saturating_add(line.len()) > MAX_BYTES {
            break;
        }
        out.push_str(&line);
    }
    out
}

// Alias keeps the match readable without importing the public module into
// every ingest call site.
use crate::workflow_input as loopal_runtime_disposition;

fn extract_title(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    let mut chars = line.chars();
    let head: String = chars.by_ref().take(80).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

#[cfg(test)]
#[path = "ingest_tests.rs"]
mod tests;
