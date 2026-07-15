use loopal_error::Result;
use loopal_protocol::{ControlCommand, GoalTransitionReason, ThreadGoal, ThreadGoalStatus};
use loopal_tool_api::GoalSessionError;
use tracing::warn;

use super::input_control::ControlOutcome;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn handle_goal_control(
        &mut self,
        ctrl: ControlCommand,
    ) -> Result<ControlOutcome> {
        let session = match self.params.goal_session.as_ref() {
            Some(s) => s.clone(),
            None => {
                warn!("goal control received but goal feature is disabled");
                return Ok(ControlOutcome::rejected("goal feature is disabled"));
            }
        };
        let kickoff_eligible = matches!(
            &ctrl,
            ControlCommand::GoalCreate { .. }
                | ControlCommand::GoalUserResume
                | ControlCommand::GoalUserReopen
        );
        let is_clear = matches!(&ctrl, ControlCommand::GoalClear);
        let outcome: std::result::Result<Option<ThreadGoal>, GoalSessionError> = match ctrl {
            ControlCommand::GoalCreate { objective } => {
                session.create_or_replace(objective).await.map(Some)
            }
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
            ControlCommand::GoalUserReopen => {
                transition(
                    &session,
                    ThreadGoalStatus::Active,
                    GoalTransitionReason::UserReopened,
                )
                .await
            }
            ControlCommand::GoalClear => session.clear().await.map(|()| None),
            other => {
                warn!(?other, "non-goal control routed through goal handler");
                return Ok(ControlOutcome::rejected("command is not a goal control"));
            }
        };
        if let Err(err) = outcome {
            warn!(error = %err, "goal control rejected");
            return Ok(ControlOutcome::rejected(format!(
                "goal control rejected: {err}"
            )));
        }
        if is_clear {
            self.reset_continuation_state();
        }
        if kickoff_eligible {
            return Ok(if self.goal_continuation_check().await? {
                ControlOutcome::continuation()
            } else {
                ControlOutcome::applied()
            });
        }
        Ok(ControlOutcome::applied())
    }
}

async fn transition(
    session: &crate::goal::GoalRuntimeSession,
    target: ThreadGoalStatus,
    reason: GoalTransitionReason,
) -> std::result::Result<Option<ThreadGoal>, GoalSessionError> {
    session.transition(target, reason).await.map(Some)
}
