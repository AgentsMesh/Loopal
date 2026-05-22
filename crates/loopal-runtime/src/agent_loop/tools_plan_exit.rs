use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_plan_mode::EXIT_PLAN_NAME;
use tracing::{debug, info, warn};

use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;
use crate::frontend::traits::PlanApproval;
use crate::mode::AgentMode;

impl AgentLoopRunner {
    pub(super) async fn handle_exit_plan(
        &mut self,
        _turn_ctx: &mut TurnContext,
        idx: usize,
        id: &str,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        debug!(tool = EXIT_PLAN_NAME, "intercepted");

        if self.params.config.mode != AgentMode::Plan {
            return Ok((
                idx,
                self.emit_and_block(
                    id,
                    EXIT_PLAN_NAME,
                    "You are not in plan mode. If your plan was already approved, \
                     continue with implementation.",
                    true,
                    None,
                )
                .await?,
            ));
        }

        let plan_content = match self.plan_file.read() {
            Some(c) => c,
            None => {
                let msg = format!(
                    "No plan file at {}. Write your plan before calling ExitPlanMode.",
                    self.plan_file.path().display()
                );
                return Ok((
                    idx,
                    self.emit_and_block(id, EXIT_PLAN_NAME, msg, true, None)
                        .await?,
                ));
            }
        };

        let plan_path_str = self.plan_file.path().to_string_lossy().to_string();
        let approval = self
            .params
            .deps
            .frontend
            .request_plan_approval(&plan_content, &plan_path_str)
            .await;

        match approval {
            PlanApproval::Approve => {
                self.restore_pre_plan_state().await?;
                self.emit_approved_result(idx, id, &plan_content).await
            }
            PlanApproval::ApproveWithEdits(edited) => {
                if let Err(e) = std::fs::write(self.plan_file.path(), &edited) {
                    warn!(error = %e, "failed to persist edited plan");
                }
                self.restore_pre_plan_state().await?;
                self.emit_approved_result(idx, id, &edited).await
            }
            PlanApproval::Reject => Ok((
                idx,
                self.emit_and_block(
                    id,
                    EXIT_PLAN_NAME,
                    "User rejected the plan. Revise and call ExitPlanMode again.",
                    false,
                    None,
                )
                .await?,
            )),
        }
    }

    async fn restore_pre_plan_state(&mut self) -> loopal_error::Result<()> {
        match self.params.config.plan_state.take() {
            Some(s) => {
                self.params.config.mode = s.previous_mode;
                self.params.config.permission_mode = s.previous_permission_mode;
            }
            None => {
                warn!("restore_pre_plan_state: no snapshot, defaulting to Act");
                self.params.config.mode = AgentMode::Act;
            }
        }
        let mode_str = match self.params.config.mode {
            AgentMode::Act => "act",
            AgentMode::Plan => "plan",
        };
        self.emit_in_turn(AgentEventPayload::ModeChanged {
            mode: mode_str.into(),
        })
        .await?;
        info!("restored pre-plan mode: {mode_str}");
        Ok(())
    }

    async fn emit_approved_result(
        &self,
        idx: usize,
        id: &str,
        plan: &str,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        let team_hint = if self.params.deps.kernel.get_tool("Agent").is_some() {
            "\n\nIf this plan can be broken into independent tasks, \
             consider using the Agent tool to parallelize."
        } else {
            ""
        };
        let path = self.plan_file.path().display();
        let content = format!(
            "User approved your plan. Start implementing.\n\n\
             Plan saved at: {path}\n\
             Refer back to it during implementation.{team_hint}\n\n\
             ## Approved Plan:\n{plan}"
        );
        Ok((
            idx,
            self.emit_and_block(id, EXIT_PLAN_NAME, content, false, None)
                .await?,
        ))
    }
}
