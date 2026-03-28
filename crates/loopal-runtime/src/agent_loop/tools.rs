use loopal_error::Result;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::AgentEventPayload;
use tracing::{debug, error, info};

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::tool_exec::execute_approved_tools;
use super::tools_inject::success_block;
use super::tools_util::{format_answers, parse_questions};
use crate::mode::AgentMode;

impl AgentLoopRunner {
    /// Execute tool calls: intercept → precheck → permission → parallel execution.
    /// Returns `Some(result)` if AttemptCompletion was called, `None` otherwise.
    pub async fn execute_tools(
        &mut self,
        tool_uses: Vec<(String, String, serde_json::Value)>,
        cancel: &TurnCancel,
    ) -> Result<Option<String>> {
        if cancel.is_cancelled() {
            return self.emit_all_interrupted(&tool_uses).await;
        }

        // Phase 0: Intercept special tools (EnterPlanMode, ExitPlanMode, AskUser)
        let (intercepted, remaining) = self.intercept_special_tools(&tool_uses).await?;

        // Phase 1: Sandbox precheck + permission checks
        info!(remaining = remaining.len(), "check_tools start");
        let check = self.check_tools(&remaining, &tool_uses, cancel).await?;
        info!(approved = check.approved.len(), denied = check.denied.len(), "check_tools done");

        // Phase 2: Parallel execution
        let mut indexed_results: Vec<(usize, ContentBlock)> = Vec::new();
        indexed_results.extend(intercepted);
        indexed_results.extend(check.denied);

        if !check.approved.is_empty() {
            if check.approved.len() >= 3 {
                let tool_ids: Vec<String> =
                    check.approved.iter().map(|(id, _, _)| id.clone()).collect();
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
        }

        self.finalize_tool_results(indexed_results)
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
                    debug!(tool = name, "intercepted special tool");
                    self.params.config.mode = AgentMode::Plan;
                    self.emit(AgentEventPayload::ModeChanged {
                        mode: "plan".into(),
                    })
                    .await?;
                    intercepted.push((
                        idx,
                        success_block(id, "Plan mode activated. Only read-only tools allowed."),
                    ));
                }
                "ExitPlanMode" => {
                    self.params.config.mode = AgentMode::Act;
                    self.emit(AgentEventPayload::ModeChanged { mode: "act".into() })
                        .await?;
                    intercepted.push((
                        idx,
                        success_block(id, "Returned to Act mode. All tools available."),
                    ));
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

    /// Sort results, detect AttemptCompletion, persist message.
    fn finalize_tool_results(
        &mut self,
        mut indexed_results: Vec<(usize, ContentBlock)>,
    ) -> Result<Option<String>> {
        indexed_results.sort_by_key(|(idx, _)| *idx);
        let blocks: Vec<ContentBlock> = indexed_results.into_iter().map(|(_, b)| b).collect();

        let completion = blocks.iter().find_map(|b| {
            if let ContentBlock::ToolResult {
                content,
                is_error: false,
                is_completion: true,
                ..
            } = b
            {
                Some(content.clone())
            } else {
                None
            }
        });

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
        Ok(completion)
    }
}
