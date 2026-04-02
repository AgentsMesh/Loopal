//! Plan mode tool interception — EnterPlanMode / ExitPlanMode logic.

use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::PermissionDecision;
use tracing::{debug, info, warn};

use super::PlanModeState;
use super::runner::AgentLoopRunner;
use super::tools_check::error_block;
use super::tools_inject::success_block;
use crate::frontend::traits::PlanApproval;
use crate::mode::AgentMode;
use crate::plan_file::build_plan_mode_filter;

impl AgentLoopRunner {
    /// Handle EnterPlanMode — validate, request consent, snapshot state, switch.
    pub(super) async fn handle_enter_plan(
        &mut self,
        idx: usize,
        id: &str,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        debug!(tool = "EnterPlanMode", "intercepted");

        if self.params.config.mode == AgentMode::Plan {
            return Ok((idx, error_block(id, "Already in plan mode.")));
        }
        if self.params.config.lifecycle == super::LifecycleMode::Ephemeral {
            return Ok((
                idx,
                error_block(id, "EnterPlanMode cannot be used in agent contexts"),
            ));
        }

        let decision = self
            .params
            .deps
            .frontend
            .request_permission(id, "EnterPlanMode", &serde_json::json!({}))
            .await;
        if decision != PermissionDecision::Allow {
            return Ok((
                idx,
                success_block(
                    id,
                    "User declined to enter plan mode. Continue without planning.",
                ),
            ));
        }

        // Atomically snapshot pre-plan state.
        self.params.config.plan_state = Some(PlanModeState {
            previous_mode: self.params.config.mode,
            previous_permission_mode: self.params.config.permission_mode,
            tool_filter: build_plan_mode_filter(&self.params.deps.kernel),
        });
        self.params.config.mode = AgentMode::Plan;

        self.emit(AgentEventPayload::ModeChanged {
            mode: "plan".into(),
        })
        .await?;

        // Ensure plans directory exists — rollback on failure to avoid dead state.
        if let Some(dir) = self.plan_file.path().parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                warn!(error = %e, "failed to create plans directory, rolling back");
                // Rollback: restore pre-plan state so agent isn't stuck in Plan mode.
                if let Some(s) = self.params.config.plan_state.take() {
                    self.params.config.mode = s.previous_mode;
                    self.params.config.permission_mode = s.previous_permission_mode;
                }
                let _ = self
                    .emit(AgentEventPayload::ModeChanged { mode: "act".into() })
                    .await;
                return Ok((
                    idx,
                    error_block(
                        id,
                        &format!("Cannot create plans directory: {e}. Plan mode was not entered."),
                    ),
                ));
            }
        }

        let plan_path = self.plan_file.path().display();
        let file_info = if self.plan_file.exists() {
            format!("A plan file already exists at {plan_path}. Read it and edit incrementally.")
        } else {
            format!("No plan file yet. Create your plan at {plan_path} using the Write tool.")
        };
        info!(plan_file = %plan_path, "entered plan mode");
        Ok((
            idx,
            success_block(
                id,
                &format!(
                    "Entered plan mode.\n\n\
             ## Plan File Info:\n{file_info}\n\
             This is the ONLY file you may edit. All other tools are read-only.\n\
             Detailed workflow instructions will follow."
                ),
            ),
        ))
    }

    /// Handle ExitPlanMode — validate, read plan, approve, restore state.
    pub(super) async fn handle_exit_plan(
        &mut self,
        idx: usize,
        id: &str,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        debug!(tool = "ExitPlanMode", "intercepted");

        if self.params.config.mode != AgentMode::Plan {
            return Ok((
                idx,
                error_block(
                    id,
                    "You are not in plan mode. If your plan was already approved, \
                 continue with implementation.",
                ),
            ));
        }

        let plan_content = match self.plan_file.read() {
            Some(c) => c,
            None => {
                return Ok((
                    idx,
                    error_block(
                        id,
                        &format!(
                            "No plan file at {}. Write your plan before calling ExitPlanMode.",
                            self.plan_file.path().display()
                        ),
                    ),
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
                Ok((idx, self.build_approved_result(id, &plan_content)))
            }
            PlanApproval::ApproveWithEdits(edited) => {
                if let Err(e) = std::fs::write(self.plan_file.path(), &edited) {
                    warn!(error = %e, "failed to persist edited plan");
                }
                self.restore_pre_plan_state().await?;
                Ok((idx, self.build_approved_result(id, &edited)))
            }
            PlanApproval::Reject => Ok((
                idx,
                success_block(
                    id,
                    "User rejected the plan. Revise and call ExitPlanMode again.",
                ),
            )),
        }
    }

    /// Restore pre-plan state atomically from the captured snapshot.
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
        self.emit(AgentEventPayload::ModeChanged {
            mode: mode_str.into(),
        })
        .await?;
        info!("restored pre-plan mode: {mode_str}");
        Ok(())
    }

    /// Build the tool_result for an approved plan.
    fn build_approved_result(&self, id: &str, plan: &str) -> ContentBlock {
        let team_hint = if self.params.deps.kernel.get_tool("Agent").is_some() {
            "\n\nIf this plan can be broken into independent tasks, \
             consider using the Agent tool to parallelize."
        } else {
            ""
        };
        let path = self.plan_file.path().display();
        success_block(
            id,
            &format!(
                "User approved your plan. Start implementing.\n\n\
             Plan saved at: {path}\n\
             Refer back to it during implementation.{team_hint}\n\n\
             ## Approved Plan:\n{plan}"
            ),
        )
    }

    /// Plan mode tool filter — delegates to the captured snapshot.
    pub(super) fn plan_tool_filter(&self) -> Option<&std::collections::HashSet<String>> {
        self.params
            .config
            .plan_state
            .as_ref()
            .map(|s| &s.tool_filter)
    }
}
