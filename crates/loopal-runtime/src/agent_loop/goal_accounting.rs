use std::sync::Arc;

use tracing::{debug, warn};

use crate::goal::UsageOutcome;
use crate::goal::prompts::render_budget_limit_prompt;

use super::runner::AgentLoopRunner;
use super::token_accumulator::TokenAccumulator;
use super::turn_context::TurnContext;

pub(crate) fn compute_token_delta_to_charge(
    cur: &TokenAccumulator,
    baseline: &TokenAccumulator,
    already_charged: u64,
) -> Option<u64> {
    // cache_read excluded from charge: hits cost ~1/10 of fresh input
    let cumulative = u64::from(
        cur.input
            .saturating_sub(cur.cache_read)
            .saturating_sub(baseline.input.saturating_sub(baseline.cache_read)),
    )
    .saturating_add(u64::from(cur.output.saturating_sub(baseline.output)))
    .saturating_add(u64::from(cur.thinking.saturating_sub(baseline.thinking)));
    if cumulative <= already_charged {
        None
    } else {
        Some(cumulative - already_charged)
    }
}

impl AgentLoopRunner {
    pub(super) async fn flush_goal_token_delta(&mut self, turn_ctx: &mut TurnContext) -> bool {
        let session = match self.params.goal_session.as_ref() {
            Some(s) => Arc::clone(s),
            None => return false,
        };
        let baseline = match turn_ctx.token_baseline.as_ref() {
            Some(b) => b,
            None => return false,
        };
        let delta = match compute_token_delta_to_charge(
            &self.tokens,
            baseline,
            turn_ctx.cumulative_charged_to_goal,
        ) {
            Some(d) => d,
            None => return false,
        };
        turn_ctx.cumulative_charged_to_goal =
            turn_ctx.cumulative_charged_to_goal.saturating_add(delta);
        match session.add_usage(delta, 0).await {
            Ok(UsageOutcome::BudgetExhausted) => true,
            Ok(_) => false,
            Err(err) => {
                warn!(error = %err, "failed to flush goal token delta");
                false
            }
        }
    }

    pub(super) async fn maybe_inject_budget_limit_warning(&mut self, turn_ctx: &mut TurnContext) {
        if turn_ctx.budget_limit_warning_pushed {
            return;
        }
        if !self.flush_goal_token_delta(turn_ctx).await {
            return;
        }
        let goal = match self.params.goal_session.as_ref() {
            Some(s) => match s.snapshot().await {
                Ok(Some(g)) => g,
                _ => return,
            },
            None => return,
        };
        turn_ctx
            .pending_warnings
            .push(render_budget_limit_prompt(&goal));
        turn_ctx.budget_limit_warning_pushed = true;
        debug!("budget_limit warning queued for current turn");
    }

    pub(super) async fn charge_goal_for_turn(
        &mut self,
        turn_ctx: &mut TurnContext,
        duration_ms: u64,
    ) {
        let _ = self.flush_goal_token_delta(turn_ctx).await;
        if duration_ms == 0 {
            return;
        }
        if let Some(session) = self.params.goal_session.as_ref()
            && let Err(err) = session.add_usage(0, duration_ms).await
        {
            warn!(error = %err, "failed to charge goal wall-clock time");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(
        input: u32,
        cache_read: u32,
        output: u32,
        thinking: u32,
        cache_creation: u32,
    ) -> TokenAccumulator {
        TokenAccumulator {
            input,
            output,
            cache_creation,
            cache_read,
            thinking,
        }
    }

    #[test]
    fn token_delta_returns_none_when_no_new_tokens() {
        let cur = acc(100, 0, 50, 0, 0);
        let baseline = cur.clone();
        assert!(compute_token_delta_to_charge(&cur, &baseline, 0).is_none());
    }

    #[test]
    fn token_delta_excludes_cache_read_from_input() {
        let baseline = acc(0, 0, 0, 0, 0);
        let cur = acc(1_000, 800, 200, 0, 0);
        assert_eq!(
            compute_token_delta_to_charge(&cur, &baseline, 0),
            Some((1_000 - 800) + 200)
        );
    }

    #[test]
    fn token_delta_subtracts_already_charged() {
        let baseline = acc(0, 0, 0, 0, 0);
        let cur = acc(1_000, 0, 200, 50, 0);
        assert_eq!(
            compute_token_delta_to_charge(&cur, &baseline, 500),
            Some(1_000 + 200 + 50 - 500)
        );
    }

    #[test]
    fn token_delta_returns_none_when_already_caught_up() {
        let baseline = acc(0, 0, 0, 0, 0);
        let cur = acc(100, 0, 50, 0, 0);
        assert!(compute_token_delta_to_charge(&cur, &baseline, 150).is_none());
    }

    #[test]
    fn token_delta_includes_thinking_tokens_face_value() {
        let baseline = acc(0, 0, 0, 0, 0);
        let cur = acc(0, 0, 0, 1_500, 0);
        assert_eq!(
            compute_token_delta_to_charge(&cur, &baseline, 0),
            Some(1_500)
        );
    }

    #[test]
    fn token_delta_relative_to_non_zero_baseline() {
        let baseline = acc(500, 100, 200, 50, 0);
        let cur = acc(800, 100, 250, 75, 0);
        // input delta after cache_read: (800-100) - (500-100) = 300
        // output delta: 50, thinking delta: 25
        assert_eq!(
            compute_token_delta_to_charge(&cur, &baseline, 0),
            Some(300 + 50 + 25)
        );
    }
}
