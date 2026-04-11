//! Tool execution orchestration: intercept → precheck → permission → execute.

use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::AgentEventPayload;
use tracing::{error, info};

use super::cancel::TurnCancel;
use super::question_parse::{format_answers, parse_questions};
use super::runner::AgentLoopRunner;
use super::streaming_tool_exec::StreamingToolHandle;
use super::tool_exec::execute_approved_tools;
use super::tools_inject::success_block;
use super::turn_metrics::ToolExecStats;
use crate::mode::AgentMode;
use crate::plan_file::wrap_plan_reminder;

use loopal_error::Result;

impl AgentLoopRunner {
    /// Execute tool calls with early-started ReadOnly results.
    ///
    /// ReadOnly tools were already spawned by `streaming_tool_exec::feed_tool`
    /// before this method is called. This method:
    /// 1. Intercepts special tools (EnterPlanMode, ExitPlanMode, AskUser)
    /// 2. Runs permission checks only for non-early tools
    /// 3. Executes non-early approved tools in parallel
    /// 4. Awaits early-started tools and merges all results
    pub async fn execute_tools_with_early(
        &mut self,
        tool_uses: Vec<(String, String, serde_json::Value)>,
        cancel: &TurnCancel,
        early_handle: StreamingToolHandle,
    ) -> Result<ToolExecStats> {
        if cancel.is_cancelled() {
            early_handle.discard();
            self.emit_all_interrupted(&tool_uses).await?;
            return Ok(ToolExecStats::default());
        }

        // Phase 0: Intercept special tools (EnterPlanMode, ExitPlanMode, AskUser)
        let (intercepted, remaining) = self.intercept_special_tools(&tool_uses).await?;
        let intercepted_indices: std::collections::HashSet<usize> =
            intercepted.iter().map(|(idx, _)| *idx).collect();

        // Phase 1: Filter out early-started tools from the remaining set.
        let early_ids = early_handle.early_started_ids().clone();
        let non_early: Vec<(String, String, serde_json::Value)> = remaining
            .into_iter()
            .filter(|(id, _, _)| !early_ids.contains(id))
            .collect();

        // Phase 1b: Sandbox precheck + permission for non-early tools only.
        // Early (ReadOnly) tools skip permission — ReadOnly is always auto-approved.
        info!(
            non_early = non_early.len(),
            early = early_ids.len(),
            "check_tools start"
        );
        let check = self.check_tools(&non_early, &tool_uses, cancel).await?;
        info!(
            approved = check.approved.len(),
            denied = check.denied.len(),
            "check_tools done"
        );

        let mut stats = ToolExecStats {
            approved: check.approved.len() as u32 + early_ids.len() as u32,
            denied: check.denied.len() as u32,
            errors: 0,
        };

        // Phase 2: Execute non-early approved tools in parallel
        let mut indexed_results: Vec<(usize, ContentBlock)> = Vec::new();
        indexed_results.extend(intercepted);
        indexed_results.extend(check.denied);

        if !check.approved.is_empty() {
            if check.approved.len() >= 3 {
                let tool_ids: Vec<String> =
                    check.approved.iter().map(|(id, _, _)| id.clone()).collect();
                let batch_id = loopal_protocol::event_id::next_event_id();
                loopal_protocol::event_id::set_current_correlation_id(batch_id);
                self.emit(AgentEventPayload::ToolBatchStart { tool_ids })
                    .await?;
            }
            let kernel = std::sync::Arc::clone(&self.params.deps.kernel);
            let tool_ctx = self.tool_ctx.clone();
            let mode = self.params.config.mode;
            let parallel = execute_approved_tools(
                check.approved,
                &tool_uses,
                kernel,
                tool_ctx,
                mode,
                &self.params.deps.frontend,
                cancel,
            )
            .await;
            indexed_results.extend(parallel);
            loopal_protocol::event_id::set_current_correlation_id(0);
        }

        // Phase 3: Collect early-started ReadOnly tool results.
        // Filter out any that were also intercepted (defensive — feed_tool already
        // skips RunnerDirect tools, but this prevents duplicate tool_result if the
        // invariant is ever broken).
        let early_results = early_handle.take_results().await;
        indexed_results.extend(
            early_results
                .into_iter()
                .filter(|(idx, _)| !intercepted_indices.contains(idx)),
        );

        // Plan mode: wrap non-intercepted tool results with system-reminder.
        if self.params.config.mode == AgentMode::Plan {
            let plan_path = self.plan_file.path().to_string_lossy().to_string();
            for (idx, block) in &mut indexed_results {
                if intercepted_indices.contains(idx) {
                    continue;
                }
                if let ContentBlock::ToolResult {
                    content, is_error, ..
                } = block
                    && !*is_error
                {
                    *content = wrap_plan_reminder(content, &plan_path);
                }
            }
        }

        for (_, block) in &indexed_results {
            if let ContentBlock::ToolResult { is_error: true, .. } = block {
                stats.errors += 1;
            }
        }

        self.finalize_tool_results(indexed_results)?;
        Ok(stats)
    }

    /// Execute tool calls: intercept → precheck → permission → parallel execution.
    ///
    /// Returns [`ToolExecStats`] for turn-level metrics aggregation.
    pub async fn execute_tools(
        &mut self,
        tool_uses: Vec<(String, String, serde_json::Value)>,
        cancel: &TurnCancel,
    ) -> Result<ToolExecStats> {
        if cancel.is_cancelled() {
            self.emit_all_interrupted(&tool_uses).await?;
            return Ok(ToolExecStats::default());
        }

        // Phase 0: Intercept special tools (EnterPlanMode, ExitPlanMode, AskUser)
        let (intercepted, remaining) = self.intercept_special_tools(&tool_uses).await?;
        // Track intercepted indices — their tool_results already contain
        // appropriate content and should not be wrapped with system-reminder.
        let intercepted_indices: std::collections::HashSet<usize> =
            intercepted.iter().map(|(idx, _)| *idx).collect();

        // Phase 1: Sandbox precheck + permission checks
        info!(remaining = remaining.len(), "check_tools start");
        let check = self.check_tools(&remaining, &tool_uses, cancel).await?;
        info!(
            approved = check.approved.len(),
            denied = check.denied.len(),
            "check_tools done"
        );

        let mut stats = ToolExecStats {
            approved: check.approved.len() as u32,
            denied: check.denied.len() as u32,
            errors: 0,
        };

        // Phase 2: Parallel execution
        let mut indexed_results: Vec<(usize, ContentBlock)> = Vec::new();
        indexed_results.extend(intercepted);
        indexed_results.extend(check.denied);

        if !check.approved.is_empty() {
            if check.approved.len() >= 3 {
                let tool_ids: Vec<String> =
                    check.approved.iter().map(|(id, _, _)| id.clone()).collect();
                // Set correlation ID so all events in this batch are grouped.
                let batch_id = loopal_protocol::event_id::next_event_id();
                loopal_protocol::event_id::set_current_correlation_id(batch_id);
                self.emit(AgentEventPayload::ToolBatchStart { tool_ids })
                    .await?;
            }
            let kernel = std::sync::Arc::clone(&self.params.deps.kernel);
            let tool_ctx = self.tool_ctx.clone();
            let mode = self.params.config.mode;
            let parallel = execute_approved_tools(
                check.approved,
                &tool_uses,
                kernel,
                tool_ctx,
                mode,
                &self.params.deps.frontend,
                cancel,
            )
            .await;
            indexed_results.extend(parallel);
            // Reset correlation ID after batch completes.
            loopal_protocol::event_id::set_current_correlation_id(0);
        }

        // Plan mode: wrap non-intercepted tool results with system-reminder.
        if self.params.config.mode == AgentMode::Plan {
            let plan_path = self.plan_file.path().to_string_lossy().to_string();
            for (idx, block) in &mut indexed_results {
                if intercepted_indices.contains(idx) {
                    continue; // Intercepted results already have appropriate content.
                }
                if let ContentBlock::ToolResult {
                    content, is_error, ..
                } = block
                    && !*is_error
                {
                    *content = wrap_plan_reminder(content, &plan_path);
                }
            }
        }

        // Count execution errors from result blocks
        for (_, block) in &indexed_results {
            if let ContentBlock::ToolResult { is_error: true, .. } = block {
                stats.errors += 1;
            }
        }

        self.finalize_tool_results(indexed_results)?;
        Ok(stats)
    }

    /// Phase 0: intercept EnterPlanMode, ExitPlanMode, AskUser.
    async fn intercept_special_tools(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<(
        Vec<(usize, ContentBlock)>,
        Vec<(String, String, serde_json::Value)>,
    )> {
        let mut intercepted = Vec::new();
        let mut remaining = Vec::new();

        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            match name.as_str() {
                "EnterPlanMode" => {
                    intercepted.push(self.handle_enter_plan(idx, id).await?);
                }
                "ExitPlanMode" => {
                    intercepted.push(self.handle_exit_plan(idx, id).await?);
                }
                "AskUser" => {
                    let questions = parse_questions(input);
                    let answers = self.params.deps.frontend.ask_user(questions).await;
                    intercepted.push((idx, success_block(id, &format_answers(&answers))));
                }
                _ => remaining.push((id.clone(), name.clone(), input.clone())),
            }
        }
        Ok((intercepted, remaining))
    }

    /// Sort results and persist message.
    fn finalize_tool_results(
        &mut self,
        mut indexed_results: Vec<(usize, ContentBlock)>,
    ) -> Result<()> {
        indexed_results.sort_by_key(|(idx, _)| *idx);
        let blocks: Vec<ContentBlock> = indexed_results.into_iter().map(|(_, b)| b).collect();

        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: blocks,
        };
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist message");
        }
        self.params.store.push_tool_results(msg);
        Ok(())
    }
}
