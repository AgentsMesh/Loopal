use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::PermissionDecision;
use loopal_tool_plan_mode::ENTER_PLAN_NAME;
use tracing::{debug, info, warn};

use super::PlanModeState;
use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;
use crate::mode::AgentMode;
use crate::plan_file::build_plan_mode_filter;

impl AgentLoopRunner {
    pub(super) async fn handle_enter_plan(
        &mut self,
        _turn_ctx: &mut TurnContext,
        idx: usize,
        id: &str,
    ) -> loopal_error::Result<(usize, ContentBlock)> {
        debug!(tool = ENTER_PLAN_NAME, "intercepted");

        if self.params.config.mode == AgentMode::Plan {
            return self
                .complete_intercepted_tool(
                    idx,
                    id,
                    ENTER_PLAN_NAME,
                    "Already in plan mode.",
                    true,
                    None,
                )
                .await;
        }
        if self.params.config.lifecycle == super::LifecycleMode::Ephemeral {
            return self
                .complete_intercepted_tool(
                    idx,
                    id,
                    ENTER_PLAN_NAME,
                    "EnterPlanMode cannot be used in agent contexts",
                    true,
                    None,
                )
                .await;
        }

        self.refresh_decision_context().await;
        let decision = self
            .params
            .deps
            .frontend
            .request_permission(id, ENTER_PLAN_NAME, &serde_json::json!({}))
            .await;
        if decision != PermissionDecision::Allow {
            return self
                .complete_intercepted_tool(
                    idx,
                    id,
                    ENTER_PLAN_NAME,
                    "User declined to enter plan mode. Continue without planning.",
                    false,
                    None,
                )
                .await;
        }

        self.params.config.plan_state = Some(PlanModeState {
            previous_mode: self.params.config.mode,
            previous_permission_mode: self.params.config.permission_mode,
            tool_filter: build_plan_mode_filter(&self.params.deps.kernel),
        });
        self.params.config.mode = AgentMode::Plan;

        self.emit_in_turn(AgentEventPayload::ModeChanged {
            mode: "plan".into(),
        })
        .await?;

        if let Some(dir) = self.plan_file.path().parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            warn!(error = %e, "failed to create plans directory, rolling back");
            if let Some(s) = self.params.config.plan_state.take() {
                self.params.config.mode = s.previous_mode;
                self.params.config.permission_mode = s.previous_permission_mode;
            }
            if let Err(emit_err) = self
                .emit_in_turn(AgentEventPayload::ModeChanged { mode: "act".into() })
                .await
            {
                tracing::error!(error = %emit_err, "ModeChanged rollback emit failed");
            }
            return self
                .complete_intercepted_tool(
                    idx,
                    id,
                    ENTER_PLAN_NAME,
                    format!("Cannot create plans directory: {e}. Plan mode was not entered."),
                    true,
                    None,
                )
                .await;
        }

        if let Some(plans_dir) = self.plan_file.path().parent()
            && let Some(loopal_dir) = plans_dir.parent()
        {
            loopal_git::ensure_loopal_gitignore(loopal_dir);
        }

        let plan_path = self.plan_file.path().display();
        let file_info = if self.plan_file.exists() {
            format!("A plan file already exists at {plan_path}. Read it and edit incrementally.")
        } else {
            format!("No plan file yet. Create your plan at {plan_path} using the Write tool.")
        };
        info!(plan_file = %plan_path, "entered plan mode");
        self.complete_intercepted_tool(
            idx,
            id,
            ENTER_PLAN_NAME,
            format!(
                "Entered plan mode.\n\n\
             ## Plan File Info:\n{file_info}\n\
             This is the ONLY file you may edit. All other tools are read-only.\n\
             Detailed workflow instructions will follow."
            ),
            false,
            None,
        )
        .await
    }

    pub(super) fn plan_tool_filter(&self) -> Option<&std::collections::HashSet<String>> {
        self.params
            .config
            .plan_state
            .as_ref()
            .map(|s| &s.tool_filter)
    }
}
