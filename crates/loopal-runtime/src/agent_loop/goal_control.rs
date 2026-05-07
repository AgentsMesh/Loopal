use loopal_error::Result;
use loopal_protocol::{ControlCommand, GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
use loopal_tool_api::GoalSessionError;
use tracing::warn;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// User-initiated goal lifecycle commands. Validates allowed transitions
    /// against the state machine; illegal targets are warned but do not abort
    /// the agent loop.
    pub(super) async fn handle_goal_control(&mut self, ctrl: ControlCommand) -> Result<()> {
        let session = match self.params.goal_session.as_ref() {
            Some(s) => s.clone(),
            None => {
                warn!("goal control received but goal feature is disabled");
                return Ok(());
            }
        };
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
            ControlCommand::GoalClear => {
                // any in-flight continuation envelope is bound to the cleared goal
                self.last_continuation_goal_id = None;
                self.barren_continuation_count = 0;
                session.clear().await.map(|()| None)
            }
            other => {
                warn!(?other, "non-goal control routed through goal handler");
                return Ok(());
            }
        };
        if let Err(err) = outcome {
            warn!(error = %err, "goal control rejected");
        }
        Ok(())
    }
}

async fn transition(
    session: &crate::goal::GoalRuntimeSession,
    target: ThreadGoalStatus,
    reason: GoalTransitionReason,
) -> std::result::Result<Option<ThreadGoal>, GoalSessionError> {
    session.transition(target, reason).await.map(Some)
}
