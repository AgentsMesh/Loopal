use loopal_protocol::{AgentStatus, Envelope};
use tracing::error;

use super::input::WaitResult;
use super::message_build::build_user_message;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn ingest_message(&mut self, env: &Envelope) -> WaitResult {
        if matches!(self.status, AgentStatus::Suspended)
            && env.source.wakes_suspended_session()
            && let Err(err) = self.transition(AgentStatus::Running).await
        {
            tracing::warn!(error = %err, "transition out of Suspended on human input failed");
        }
        let was_closed = !self.continuation_gate.is_open();
        self.continuation_gate.open_for_envelope();
        if was_closed {
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
        let mut user_msg = build_user_message(env);
        let ephemeral = env.source.is_ephemeral_in_history();
        user_msg.ephemeral_in_history = ephemeral;
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut user_msg)
        {
            error!(error = %e, "failed to persist message");
        }
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
        self.params.store.push_user(user_msg);

        let message_id = env.id.to_string();
        // reason: emit before tracking the id — a failed emit must not leave
        // an orphan InboxConsumed without its enqueued counterpart.
        match self
            .emit(loopal_protocol::AgentEventPayload::InboxEnqueued {
                message_id: message_id.clone(),
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

        WaitResult::MessageAdded
    }
}

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
