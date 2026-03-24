use loopal_error::Result;
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::AgentEventPayload;
use tracing::{error, info};

use loopal_tool_api::COMPLETION_PREFIX;

use super::cancel::TurnCancel;
use super::input::build_user_message;
use super::runner::AgentLoopRunner;
use super::tool_exec::execute_approved_tools;
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
        let check = self.check_tools(&remaining, &tool_uses, cancel).await?;

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
            let kernel = std::sync::Arc::clone(&self.params.kernel);
            let tool_ctx = self.tool_ctx.clone();
            let mode = self.params.mode;
            let parallel = execute_approved_tools(
                check.approved,
                &tool_uses,
                kernel,
                tool_ctx,
                mode,
                &self.params.frontend,
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
                    self.params.mode = AgentMode::Plan;
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
                    self.params.mode = AgentMode::Act;
                    self.emit(AgentEventPayload::ModeChanged { mode: "act".into() })
                        .await?;
                    intercepted.push((
                        idx,
                        success_block(id, "Returned to Act mode. All tools available."),
                    ));
                }
                "AskUser" => {
                    let questions = parse_questions(input);
                    let answers = self.params.frontend.ask_user(questions).await;
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
                ..
            } = b
            {
                content
                    .strip_prefix(COMPLETION_PREFIX)
                    .map(|s| s.to_string())
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
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist message");
        }
        self.params.store.push_tool_results(msg);
        Ok(completion)
    }

    /// Emit interrupted results for all tools (early cancel path).
    async fn emit_all_interrupted(
        &mut self,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<Option<String>> {
        info!("cancelled, skipping tool execution");
        let mut blocks = Vec::with_capacity(tool_uses.len());
        for (id, name, _) in tool_uses {
            self.emit(AgentEventPayload::ToolResult {
                id: id.clone(),
                name: name.clone(),
                result: "Interrupted by user".into(),
                is_error: true,
                duration_ms: None,
            })
            .await?;
            blocks.push(ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: "Interrupted by user".into(),
                is_error: true,
            });
        }
        let mut msg = Message {
            id: None,
            role: MessageRole::User,
            content: blocks,
        };
        if let Err(e) = self
            .params
            .session_manager
            .save_message(&self.params.session.id, &mut msg)
        {
            error!(error = %e, "failed to persist message");
        }
        self.params.store.push_tool_results(msg);
        Ok(None)
    }

    /// Drain pending envelopes from the frontend and inject them as user messages.
    pub async fn inject_pending_messages(&mut self) {
        let pending = self.params.frontend.drain_pending().await;
        for env in pending {
            let mut user_msg = build_user_message(&env);
            info!(
                text_len = env.content.text.len(),
                "injecting pending message"
            );
            if let Err(e) = self
                .params
                .session_manager
                .save_message(&self.params.session.id, &mut user_msg)
            {
                error!(error = %e, "failed to persist injected message");
            }
            self.params.store.push_user(user_msg);
        }
    }
}

fn success_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: false,
    }
}
