//! Turn-level orchestration: observer dispatch, telemetry, and event emission.

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use opentelemetry::KeyValue;
use tracing::{Instrument, info, info_span};

use super::TurnOutput;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn execute_turn(&mut self, turn_ctx: &mut TurnContext) -> Result<TurnOutput> {
        let span = info_span!(
            "turn",
            loopal.turn.id = turn_ctx.turn_id,
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
        for h in &mut self.hooks {
            h.on_turn_start(turn_ctx);
        }
        let result = self.execute_turn_inner(turn_ctx).await;
        for h in &mut self.hooks {
            h.on_turn_end(turn_ctx);
        }

        turn_ctx.metrics.warnings_injected = turn_ctx.pending_warnings.len() as u32;
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
        let model_attr = KeyValue::new(
            "gen_ai.request.model",
            self.params.config.model().to_string(),
        );
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

        if let Err(emit_err) = self
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

        self.record_turn_for_barren_tracking(&turn_ctx.metrics);

        result
    }
}
