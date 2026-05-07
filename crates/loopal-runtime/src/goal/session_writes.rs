use loopal_protocol::{GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
use loopal_tool_api::GoalSessionError;

use super::session::{GoalRuntimeSession, UsageOutcome};

enum AddUsageResult {
    Updated(ThreadGoal),
    BudgetExhausted(ThreadGoal),
}

impl GoalRuntimeSession {
    pub async fn add_usage(
        &self,
        delta_tokens: u64,
        delta_time_ms: u64,
    ) -> Result<UsageOutcome, GoalSessionError> {
        if delta_tokens == 0 && delta_time_ms == 0 {
            return Ok(UsageOutcome::NoOp);
        }
        let result = {
            let _guard = self.write_lock.lock().await;
            let id = self.current_session_id();
            let mut goal = match self
                .store
                .load(&id)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?
            {
                Some(g) => g,
                None => return Ok(UsageOutcome::NoOp),
            };
            if !goal.status.participates_in_continuation() {
                return Ok(UsageOutcome::NoOp);
            }
            goal.tokens_used = goal.tokens_used.saturating_add(delta_tokens);
            goal.time_used_ms = goal.time_used_ms.saturating_add(delta_time_ms);
            goal.updated_at = chrono::Utc::now();
            let exhausted = goal.budget_exhausted();
            if exhausted {
                goal.status = ThreadGoalStatus::BudgetLimited;
            }
            self.store
                .save(&goal)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?;
            if exhausted {
                AddUsageResult::BudgetExhausted(goal)
            } else {
                AddUsageResult::Updated(goal)
            }
        };
        match result {
            AddUsageResult::Updated(g) => {
                self.emit_updated(Some(g), GoalTransitionReason::UsageUpdated)
                    .await;
                Ok(UsageOutcome::Updated)
            }
            AddUsageResult::BudgetExhausted(g) => {
                self.emit_updated(Some(g), GoalTransitionReason::BudgetExhausted)
                    .await;
                Ok(UsageOutcome::BudgetExhausted)
            }
        }
    }

    /// Atomic BudgetLimited → Active transition with budget bump.
    pub async fn extend_budget(
        &self,
        additional_tokens: u64,
    ) -> Result<ThreadGoal, GoalSessionError> {
        if additional_tokens == 0 {
            return Err(GoalSessionError::InvalidBudget);
        }
        let goal = {
            let _guard = self.write_lock.lock().await;
            let id = self.current_session_id();
            let mut goal = self
                .store
                .load(&id)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?
                .ok_or(GoalSessionError::NotFound)?;
            if !goal.status.can_transition_to(
                ThreadGoalStatus::Active,
                GoalTransitionReason::UserExtendedBudget,
            ) {
                return Err(GoalSessionError::ModelStatusForbidden);
            }
            let prev_budget = goal.token_budget.unwrap_or(goal.tokens_used);
            goal.token_budget = Some(prev_budget.saturating_add(additional_tokens));
            goal.status = ThreadGoalStatus::Active;
            goal.updated_at = chrono::Utc::now();
            self.store
                .save(&goal)
                .map_err(|e| GoalSessionError::Storage(e.to_string()))?;
            goal
        };
        self.emit_updated(Some(goal.clone()), GoalTransitionReason::UserExtendedBudget)
            .await;
        Ok(goal)
    }
}
