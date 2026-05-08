use loopal_error::Result;
use loopal_protocol::{ControlCommand, GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
use loopal_tool_api::GoalSessionError;
use tracing::warn;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// User-initiated goal lifecycle commands. Returns `true` when the
    /// command transitions the goal into `Active` and a continuation
    /// envelope was injected — callers in idle context must treat this as
    /// "input added" and exit `wait_for_input` immediately.
    pub(super) async fn handle_goal_control(&mut self, ctrl: ControlCommand) -> Result<bool> {
        let session = match self.params.goal_session.as_ref() {
            Some(s) => s.clone(),
            None => {
                warn!("goal control received but goal feature is disabled");
                return Ok(false);
            }
        };
        let kickoff_eligible = matches!(
            &ctrl,
            ControlCommand::GoalCreate { .. }
                | ControlCommand::GoalUserResume
                | ControlCommand::GoalExtendBudget { .. }
        );
        let is_clear = matches!(&ctrl, ControlCommand::GoalClear);
        let outcome: std::result::Result<Option<ThreadGoal>, GoalSessionError> = match ctrl {
            ControlCommand::GoalCreate {
                objective,
                token_budget,
            } => session.create(objective, token_budget).await.map(Some),
            ControlCommand::GoalUserPause => {
                transition(
                    &session,
                    ThreadGoalStatus::Paused,
                    GoalTransitionReason::UserPaused,
                )
                .await
            }
            ControlCommand::GoalUserResume => {
                transition(
                    &session,
                    ThreadGoalStatus::Active,
                    GoalTransitionReason::UserResumed,
                )
                .await
            }
            ControlCommand::GoalUserComplete => {
                transition(
                    &session,
                    ThreadGoalStatus::Complete,
                    GoalTransitionReason::UserCompleted,
                )
                .await
            }
            ControlCommand::GoalExtendBudget { additional_tokens } => {
                session.extend_budget(additional_tokens).await.map(Some)
            }
            ControlCommand::GoalClear => session.clear().await.map(|()| None),
            other => {
                warn!(?other, "non-goal control routed through goal handler");
                return Ok(false);
            }
        };
        if let Err(err) = outcome {
            warn!(error = %err, "goal control rejected");
            return Ok(false);
        }
        if is_clear {
            // Only after clear() succeeded — failed clears must not orphan
            // the runner's continuation tracking from the on-disk goal.
            self.last_continuation_goal_id = None;
            self.barren_continuation_count = 0;
        }
        if kickoff_eligible {
            // Resume restarts the barren window too: pause/resume implies
            // "try again", aligned with create/extend semantics.
            self.barren_continuation_count = 0;
            return self.goal_continuation_check().await;
        }
        Ok(false)
    }
}

async fn transition(
    session: &crate::goal::GoalRuntimeSession,
    target: ThreadGoalStatus,
    reason: GoalTransitionReason,
) -> std::result::Result<Option<ThreadGoal>, GoalSessionError> {
    session.transition(target, reason).await.map(Some)
}
