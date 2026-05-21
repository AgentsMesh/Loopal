use chrono::{Duration, Utc};
use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_idle::{MAX_IDLE_DURATION_SECS, MIN_IDLE_DURATION_SECS, validate_duration};
use serde::Deserialize;
use tracing::debug;

use super::continuation_gate::GateClose;
use super::runner::AgentLoopRunner;
use super::tools_inject::tool_result_block;

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
        idx: usize,
        id: &str,
        input: &serde_json::Value,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        debug!(tool = "request_idle", "intercepted");
        let parsed: ParsedIdle = match serde_json::from_value(input.clone()) {
            Ok(p) => p,
            Err(e) => {
                return Ok((
                    idx,
                    tool_result_block(
                        id,
                        &format!(
                            "request_idle: invalid arguments ({e}). Required: \
                             max_idle_duration_secs (u64, {MIN_IDLE_DURATION_SECS}..={MAX_IDLE_DURATION_SECS}), \
                             reason (string). Optional: expected_wake_signal (string)."
                        ),
                        true,
                        None,
                    ),
                ));
            }
        };
        if let Err(msg) = validate_duration(parsed.max_idle_duration_secs) {
            return Ok((idx, tool_result_block(id, &msg, true, None)));
        }
        let wake_at = Utc::now() + Duration::seconds(parsed.max_idle_duration_secs as i64);
        self.continuation_gate
            .close(GateClose::ModelRequested { wake_at });
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
        Ok((idx, tool_result_block(id, &message, false, None)))
    }
}
