use std::sync::Arc;

use chrono::Utc;
use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::MessageRole;
use tracing::warn;

use crate::goal::prompts::build_continuation_envelope;
use crate::mode::AgentMode;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn goal_continuation_check(&mut self) -> Result<bool> {
        let now = Utc::now();
        if self.continuation_gate.maybe_open_on_deadline(now) {
            let summary = self.continuation_gate.summary();
            if let Err(err) = self
                .emit(AgentEventPayload::ContinuationGateChanged(summary))
                .await
            {
                tracing::error!(error = %err, "ContinuationGateChanged emit failed");
            }
        }
        if !self.continuation_gate.is_open() {
            return Ok(false);
        }
        let session = match self.params.goal_session.as_ref() {
            Some(s) => Arc::clone(s),
            None => return Ok(false),
        };
        if matches!(self.params.config.mode, AgentMode::Plan) {
            return Ok(false);
        }
        if matches!(self.params.store.last_role(), Some(MessageRole::User)) {
            return Ok(false);
        }
        let goal = match session.snapshot().await {
            Ok(Some(g)) => g,
            Ok(None) => return Ok(false),
            Err(err) => {
                warn!(error = %err, "failed to read goal for continuation");
                return Ok(false);
            }
        };
        if !goal.status.participates_in_continuation() {
            return Ok(false);
        }
        let env = build_continuation_envelope(&goal);
        self.ingest_message(&env).await;
        self.last_continuation_goal_id = Some(goal.goal_id);
        Ok(true)
    }
}
