use loopal_error::Result;
use loopal_message::MessageRole;
use loopal_provider_api::ContinuationIntent;
use tracing::info;

use super::TurnOutput;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;
use super::turn_response::TurnLoopCounters;
use super::turn_state::TurnState;

impl AgentLoopRunner {
    pub(super) async fn execute_turn_inner(
        &mut self,
        turn_ctx: &mut TurnContext,
    ) -> Result<TurnOutput> {
        let mut c = TurnLoopCounters {
            last_text: String::new(),
            continuation_count: 0,
            stop_feedback_count: 0,
            max_continuations: self.params.harness.max_auto_continuations,
            max_stop_feedback: self.params.harness.max_stop_feedback,
        };
        let mut state = TurnState::ReadyToCall;

        loop {
            state = match state {
                TurnState::Cancelled | TurnState::Complete => {
                    return Ok(TurnOutput {
                        output: c.last_text,
                    });
                }
                TurnState::ReadyToCall => self.step_ready_to_call(turn_ctx).await?,
                TurnState::ResponseRecorded { result } => {
                    self.handle_response_recorded(turn_ctx, result, &mut c)
                        .await?
                }
                TurnState::NeedsContinuation { reason } => {
                    turn_ctx.pending_continuation =
                        Some(ContinuationIntent::AutoContinue { reason });
                    TurnState::ReadyToCall
                }
                TurnState::NeedsToolExecution { tool_uses } => {
                    if self.run_before_tools(turn_ctx, &tool_uses).await? {
                        TurnState::Complete
                    } else {
                        self.execute_tool_phase(turn_ctx, tool_uses).await?;
                        TurnState::ToolResultsWritten
                    }
                }
                TurnState::NeedsStopFeedback { feedback } => {
                    self.push_stop_feedback(feedback);
                    TurnState::ReadyToCall
                }
                TurnState::ToolResultsWritten => {
                    c.continuation_count = 0;
                    TurnState::ReadyToCall
                }
            };
        }
    }

    async fn step_ready_to_call(&mut self, turn_ctx: &mut TurnContext) -> Result<TurnState> {
        if turn_ctx.cancel.is_cancelled() {
            info!("turn cancelled before LLM call");
            return Ok(TurnState::Cancelled);
        }
        debug_assert!(
            self.params.store.last_role() == Some(MessageRole::User)
                || turn_ctx.pending_continuation.is_some(),
            "ReadyToCall invariant violated: last_role={:?}, pending_continuation={}",
            self.params.store.last_role(),
            turn_ctx.pending_continuation.is_some()
        );
        self.check_and_compact().await?;
        let mut working = self.params.store.prepare_for_llm();
        self.run_context_pipeline(&mut working).await;
        turn_ctx.metrics.llm_calls += 1;
        let intent = turn_ctx.pending_continuation.take();
        let result = self
            .stream_llm_with(&working, intent, &turn_ctx.cancel)
            .await?;
        Ok(TurnState::ResponseRecorded { result })
    }
}
