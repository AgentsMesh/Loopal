use std::sync::Arc;

use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, Question, QuestionOption};
use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_tool_api::PermissionDecision;
use tracing::{error, info};

use loopal_tool_api::COMPLETION_PREFIX;

use crate::mode::AgentMode;
use super::input::build_user_message;
use super::runner::AgentLoopRunner;
use super::tool_exec::execute_approved_tools;

impl AgentLoopRunner {
    /// Execute tool calls: intercept → precheck → permission → parallel execution.
    /// Returns `Some(result)` if AttemptCompletion was called, `None` otherwise.
    pub async fn execute_tools(
        &mut self,
        tool_uses: Vec<(String, String, serde_json::Value)>,
    ) -> Result<Option<String>> {
        // Check interrupt before starting any tool execution
        if self.interrupt.is_signaled() {
            info!("interrupt signaled, skipping tool execution");
            let mut blocks = Vec::with_capacity(tool_uses.len());
            for (id, name, _) in &tool_uses {
                self.emit(AgentEventPayload::ToolResult {
                    id: id.clone(), name: name.clone(),
                    result: "Interrupted by user".into(), is_error: true,
                }).await?;
                blocks.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(), content: "Interrupted by user".into(), is_error: true,
                });
            }
            let mut msg = Message { id: None, role: MessageRole::User, content: blocks };
            if let Err(e) = self.params.session_manager.save_message(&self.params.session.id, &mut msg) {
                error!(error = %e, "failed to persist message");
            }
            self.params.messages.push(msg);
            return Ok(None);
        }

        // Phase 0: Intercept special tools (EnterPlanMode, ExitPlanMode, AskUser)
        let mut intercepted: Vec<(usize, ContentBlock)> = Vec::new();
        let mut remaining: Vec<(String, String, serde_json::Value)> = Vec::new();

        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            match name.as_str() {
                "EnterPlanMode" => {
                    self.params.mode = AgentMode::Plan;
                    self.emit(AgentEventPayload::ModeChanged { mode: "plan".into() }).await?;
                    intercepted.push((idx, tool_result_block(id, "Plan mode activated. Only read-only tools allowed.")));
                }
                "ExitPlanMode" => {
                    self.params.mode = AgentMode::Act;
                    self.emit(AgentEventPayload::ModeChanged { mode: "act".into() }).await?;
                    intercepted.push((idx, tool_result_block(id, "Returned to Act mode. All tools available.")));
                }
                "AskUser" => {
                    let questions = parse_questions(input);
                    let answers = self.params.frontend.ask_user(questions).await;
                    let formatted = format_answers(&answers);
                    intercepted.push((idx, tool_result_block(id, &formatted)));
                }
                _ => remaining.push((id.clone(), name.clone(), input.clone())),
            }
        }

        // Phase 1: Sandbox precheck then permission checks (remaining tools only)
        let mut approved: Vec<(String, String, serde_json::Value)> = Vec::new();
        let mut denied_results: Vec<(usize, ContentBlock)> = Vec::new();

        for (id, name, input) in &remaining {
            let orig_idx = tool_uses.iter().position(|(tid, _, _)| tid == id).unwrap_or(0);
            let precheck_reason = self.params.kernel
                .get_tool(name)
                .and_then(|tool| tool.precheck(input));

            if let Some(reason) = precheck_reason {
                info!(tool = name.as_str(), reason = %reason, "sandbox rejected");
                denied_results.push((orig_idx, ContentBlock::ToolResult {
                    tool_use_id: id.clone(), content: format!("Sandbox: {reason}"), is_error: true,
                }));
                self.emit(AgentEventPayload::ToolResult {
                    id: id.clone(), name: name.clone(),
                    result: format!("Sandbox: {reason}"), is_error: true,
                }).await?;
                continue;
            }

            let decision = self.check_permission(id, name, input).await?;
            if decision == PermissionDecision::Deny {
                info!(tool = name.as_str(), decision = "deny", "permission");
                denied_results.push((orig_idx, ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: format!("Permission denied: tool '{}' not allowed", name),
                    is_error: true,
                }));
                self.emit(AgentEventPayload::ToolResult {
                    id: id.clone(), name: name.clone(),
                    result: "Permission denied".to_string(), is_error: true,
                }).await?;
            } else {
                approved.push((id.clone(), name.clone(), input.clone()));
            }
        }

        // Phase 2: Parallel execution via tool_exec
        let mut indexed_results: Vec<(usize, ContentBlock)> = Vec::new();
        indexed_results.extend(intercepted);
        indexed_results.extend(denied_results);

        if !approved.is_empty() {
            let kernel = Arc::clone(&self.params.kernel);
            let tool_ctx = self.tool_ctx.clone();
            let mode = self.params.mode;
            let parallel = execute_approved_tools(
                approved, &tool_uses, kernel, tool_ctx, mode, &self.params.frontend,
            ).await;
            indexed_results.extend(parallel);
        }

        indexed_results.sort_by_key(|(idx, _)| *idx);
        let tool_result_blocks: Vec<ContentBlock> = indexed_results
            .into_iter().map(|(_, block)| block).collect();

        // Detect AttemptCompletion
        let mut completion_result: Option<String> = None;
        for block in &tool_result_blocks {
            if let ContentBlock::ToolResult { content, is_error: false, .. } = block
                && let Some(rest) = content.strip_prefix(COMPLETION_PREFIX)
            {
                completion_result = Some(rest.to_string());
            }
        }

        let mut tool_results_msg = Message { id: None, role: MessageRole::User, content: tool_result_blocks };
        if let Err(e) = self.params.session_manager.save_message(&self.params.session.id, &mut tool_results_msg) {
            error!(error = %e, "failed to persist message");
        }
        self.params.messages.push(tool_results_msg);

        Ok(completion_result)
    }

    /// Drain pending envelopes from the frontend and inject them as user messages.
    pub async fn inject_pending_messages(&mut self) {
        let pending = self.params.frontend.drain_pending().await;
        for env in pending {
            let mut user_msg = build_user_message(&env);
            info!(text_len = env.content.text.len(), "injecting pending message");
            if let Err(e) = self.params.session_manager.save_message(
                &self.params.session.id,
                &mut user_msg,
            ) {
                error!(error = %e, "failed to persist injected message");
            }
            self.params.messages.push(user_msg);
        }
    }
}

fn tool_result_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult { tool_use_id: id.to_string(), content: content.to_string(), is_error: false }
}

fn parse_questions(input: &serde_json::Value) -> Vec<Question> {
    let Some(questions) = input.get("questions").and_then(|v| v.as_array()) else {
        return vec![Question { question: "?".into(), options: Vec::new(), allow_multiple: false }];
    };
    questions.iter().map(|q| {
        let question = q.get("question").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let allow_multiple = q.get("multiSelect").and_then(|v| v.as_bool()).unwrap_or(false);
        let options = q.get("options").and_then(|v| v.as_array()).map(|arr| {
            arr.iter().map(|o| QuestionOption {
                label: o.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                description: o.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            }).collect()
        }).unwrap_or_default();
        Question { question, options, allow_multiple }
    }).collect()
}

fn format_answers(answers: &[String]) -> String {
    if answers.is_empty() { return "(no selection)".to_string(); }
    answers.join(", ")
}
