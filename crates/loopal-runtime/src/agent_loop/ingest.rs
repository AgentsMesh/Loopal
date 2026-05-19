use loopal_protocol::{Envelope, MessageSource};
use tracing::error;

use super::input::WaitResult;
use super::message_build::build_user_message;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn ingest_message(&mut self, env: &Envelope) -> WaitResult {
        let mut user_msg = build_user_message(env);
        let ephemeral = matches!(
            env.source,
            MessageSource::Scheduled | MessageSource::System(_)
        );
        if !ephemeral
            && let Err(e) = self
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
