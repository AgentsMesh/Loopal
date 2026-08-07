//! Turn-level orchestration: observer dispatch, telemetry, and event emission.

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use opentelemetry::KeyValue;
use tracing::{Instrument, info, info_span};

use super::TurnOutput;
use super::continuation_gate::GateClose;
use super::governance::bridge::DataPlaneBridge;
use super::governance::system_note::make_governance_feedback;
use super::governance::traits::PostTurnAction;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;
use super::turn_history::TurnRecord;

fn telemetry_turn_uuid(turn_id: Option<&loopal_turn::TurnId>) -> &str {
    let Some(turn_id) = turn_id else {
        return "unrecorded";
    };
    let journal_id = turn_id.as_str();
    journal_id
        .strip_prefix("t-")
        .filter(|candidate| uuid::Uuid::parse_str(candidate).is_ok())
        .unwrap_or(journal_id)
}

impl AgentLoopRunner {
    pub(super) async fn execute_turn(&mut self, turn_ctx: &mut TurnContext) -> Result<TurnOutput> {
        let journal_turn_uuid = telemetry_turn_uuid(self.turns.current_turn_id());
        let span = info_span!(
            "turn",
            loopal.turn.id = turn_ctx.turn_id,
            loopal.turn.uuid = %journal_turn_uuid,
            gen_ai.request.model = %self.params.config.model(),
        );
        let turn_id = turn_ctx.turn_id;
        loopal_protocol::event_id::scope_turn(
            turn_id,
            self.execute_turn_body(turn_ctx).instrument(span),
        )
        .await
    }

    async fn execute_turn_body(&mut self, turn_ctx: &mut TurnContext) -> Result<TurnOutput> {
        crate::otel_metrics::active_turns().add(1, &[]);
        // Skip before on_turn_start/record so a stale continuation never enters
        // turn_history (zero metrics would pollute degeneration streaks).
        if self.skip_stale_continuation_turn(turn_ctx).await {
            crate::otel_metrics::active_turns().add(-1, &[]);
            return Ok(TurnOutput {
                output: String::new(),
            });
        }
        for h in &mut self.hooks {
            h.on_turn_start(turn_ctx);
        }
        let result = self.execute_turn_inner(turn_ctx).await;
        for h in &mut self.hooks {
            h.on_turn_end(turn_ctx);
        }

        // warnings_injected accumulates at each tool-phase drain; pending_warnings
        // is empty again by turn end, so recomputing from it here would zero it.
        turn_ctx.metrics.tokens_in = self.tokens.input;
        turn_ctx.metrics.tokens_out = self.tokens.output;
        let m = &turn_ctx.metrics;
        let files: Vec<String> = turn_ctx.modified_files.iter().cloned().collect();
        let turn_duration = turn_ctx.started_at.elapsed();
        let duration_ms = turn_duration.as_millis() as u64;
        info!(
            turn = turn_ctx.turn_id,
            duration_ms,
            llm = m.llm_calls,
            tools = m.tool_calls_requested,
            ok = m.tool_calls_approved,
            denied = m.tool_calls_denied,
            errs = m.tool_errors,
            tok_in = m.tokens_in,
            tok_out = m.tokens_out,
            "turn completed"
        );

        crate::otel_metrics::active_turns().add(-1, &[]);
        let model_attr = KeyValue::new("gen_ai.request.model", self.params.config.model());
        crate::otel_metrics::turn_duration().record(
            turn_duration.as_secs_f64(),
            std::slice::from_ref(&model_attr),
        );
        let attrs = &[
            KeyValue::new("gen_ai.token.type", "input"),
            model_attr.clone(),
        ];
        crate::otel_metrics::token_usage().add(m.tokens_in as u64, attrs);
        let attrs = &[KeyValue::new("gen_ai.token.type", "output"), model_attr];
        crate::otel_metrics::token_usage().add(m.tokens_out as u64, attrs);

        // A cancelled turn is reported via TurnCancelled (finalize), not
        // TurnCompleted — emitting both would give the same turn_id divergent
        // terminal states. OTel duration/token above still record the real cost.
        if !turn_ctx.cancel.is_cancelled()
            && let Err(emit_err) = self
                .emit_in_turn(AgentEventPayload::TurnCompleted(
                    loopal_protocol::TurnSummary {
                        turn_id: turn_ctx.turn_id,
                        duration_ms,
                        llm_calls: m.llm_calls,
                        tool_calls_requested: m.tool_calls_requested,
                        tool_calls_approved: m.tool_calls_approved,
                        tool_calls_denied: m.tool_calls_denied,
                        tool_errors: m.tool_errors,
                        auto_continuations: m.auto_continuations,
                        warnings_injected: m.warnings_injected,
                        tokens_in: m.tokens_in,
                        tokens_out: m.tokens_out,
                        modified_files: files,
                    },
                ))
                .await
        {
            tracing::error!(
                error = %emit_err,
                turn_id = turn_ctx.turn_id,
                "agent_loop::turn_telemetry TurnCompleted emit failed — view-state reconcile will not trigger"
            );
        }

        self.record_turn_completion(turn_ctx).await;

        result
    }

    async fn record_turn_completion(&mut self, turn_ctx: &TurnContext) {
        // A cancelled turn is not a degeneration/loop sample: it was interrupted,
        // not the agent making (or failing to make) progress. tool_calls_approved
        // counts permitted-not-completed tools, so it cannot stand in for progress.
        if turn_ctx.cancel.is_cancelled() {
            return;
        }
        let record = TurnRecord {
            metrics: turn_ctx.metrics.clone(),
            text_hash: turn_ctx.metrics.text_hash,
        };
        self.turn_history.push(record.clone());
        let actions: Vec<PostTurnAction> = {
            let history = &self.turn_history;
            self.governance
                .iter_mut()
                .map(|g| g.on_after_turn(&record, history))
                .collect()
        };
        for action in actions {
            self.apply_post_turn_action(action).await;
        }
    }

    async fn apply_post_turn_action(&mut self, action: PostTurnAction) {
        match action {
            PostTurnAction::None => {}
            PostTurnAction::Degeneration {
                summary,
                feedback_to_model,
            } => {
                self.continuation_gate.close(GateClose::Degeneration {
                    wake_at: summary.wake_deadline,
                });
                if let Err(err) = self
                    .emit(AgentEventPayload::DegenerationDetected(summary))
                    .await
                {
                    tracing::error!(error = %err, "DegenerationDetected emit failed");
                }
                let gate = self.continuation_gate.summary();
                if let Err(err) = self
                    .emit(AgentEventPayload::ContinuationGateChanged(gate))
                    .await
                {
                    tracing::error!(error = %err, "ContinuationGateChanged emit failed");
                }
                if let Some(msg) = make_governance_feedback(&feedback_to_model) {
                    self.push_system_note(msg);
                }
            }
        }
    }
}

#[cfg(test)]
mod telemetry_tests {
    use super::telemetry_turn_uuid;
    use loopal_turn::TurnId;

    #[test]
    fn turn_uuid_strips_only_a_valid_generated_prefix() {
        let raw = "a681302d-bda9-45ac-bb60-d2dd76b663bc";
        let generated = TurnId::from_string(format!("t-{raw}"));
        assert_eq!(telemetry_turn_uuid(Some(&generated)), raw);

        let legacy = TurnId::from_string("legacy-turn-id");
        assert_eq!(telemetry_turn_uuid(Some(&legacy)), "legacy-turn-id");

        let malformed = TurnId::from_string("t-not-a-uuid");
        assert_eq!(telemetry_turn_uuid(Some(&malformed)), "t-not-a-uuid");
        assert_eq!(telemetry_turn_uuid(None), "unrecorded");
    }
}
