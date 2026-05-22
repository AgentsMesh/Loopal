use chrono::{Duration, Utc};
use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_idle::{MAX_IDLE_DURATION_SECS, MIN_IDLE_DURATION_SECS, NAME, validate_duration};
use serde::Deserialize;
use tracing::debug;

use super::continuation_gate::GateClose;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

#[derive(Deserialize)]
struct ParsedIdle {
    #[serde(default)]
    reason: String,
    max_idle_duration_secs: u64,
    #[serde(default)]
    expected_wake_signal: Option<String>,
}

impl AgentLoopRunner {
    pub(super) async fn handle_request_idle(
        &mut self,
        turn_ctx: &mut TurnContext,
        idx: usize,
        id: &str,
        input: &serde_json::Value,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        debug!(tool = NAME, "intercepted");
        let parsed: ParsedIdle = match serde_json::from_value(input.clone()) {
            Ok(p) => p,
            Err(e) => {
                let msg = format!(
                    "request_idle: invalid arguments ({e}). Required: \
                     max_idle_duration_secs (u64, {MIN_IDLE_DURATION_SECS}..={MAX_IDLE_DURATION_SECS}), \
                     reason (string). Optional: expected_wake_signal (string)."
                );
                return Ok((idx, self.emit_and_block(id, NAME, msg, true, None).await?));
            }
        };
        if let Err(msg) = validate_duration(parsed.max_idle_duration_secs) {
            return Ok((idx, self.emit_and_block(id, NAME, msg, true, None).await?));
        }
        let wake_at = Utc::now() + Duration::seconds(parsed.max_idle_duration_secs as i64);
        self.continuation_gate
            .close(GateClose::ModelRequested { wake_at });
        turn_ctx.signal_turn_end_after_tools();
        let gate = self.continuation_gate.summary();
        if let Err(err) = self
            .emit(AgentEventPayload::ContinuationGateChanged(gate))
            .await
        {
            tracing::warn!(error = %err, "ContinuationGateChanged emit failed on request_idle");
        }
        let signal = parsed
            .expected_wake_signal
            .as_deref()
            .unwrap_or("external envelope or deadline");
        let message = format!(
            "idle acknowledged until {wake_at}; runtime will reopen the continuation gate \
             at that time or when any external envelope (user/cron/hook) arrives. \
             expected_wake_signal: {signal}. reason logged: {reason}",
            wake_at = wake_at.to_rfc3339(),
            reason = parsed.reason,
        );
        Ok((
            idx,
            self.emit_and_block(id, NAME, message, false, None).await?,
        ))
    }
}
