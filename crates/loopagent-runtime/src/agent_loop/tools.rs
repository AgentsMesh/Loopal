use std::sync::Arc;
use std::time::Instant;

use loopagent_types::error::Result;
use loopagent_types::event::AgentEvent;
use loopagent_types::message::{ContentBlock, Message, MessageRole};
use loopagent_types::permission::PermissionDecision;
use tracing::{error, info};

use crate::tool_pipeline::execute_tool;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Execute tool calls with parallel execution.
    /// Phase 1: Sequential permission checks (requires user interaction).
    /// Phase 2: Parallel execution of approved tools via JoinSet.
    pub(crate) async fn execute_tools(
        &mut self,
        tool_uses: Vec<(String, String, serde_json::Value)>,
    ) -> Result<()> {
        // Phase 1: Sequential permission checks
        let mut approved: Vec<(String, String, serde_json::Value)> = Vec::new();
        let mut denied_results: Vec<(usize, ContentBlock)> = Vec::new();

        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            let decision = self.check_permission(id, name, input).await?;

            if decision == PermissionDecision::Deny {
                info!(tool = name.as_str(), decision = "deny", "permission");
                denied_results.push((idx, ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: format!("Permission denied: tool '{}' not allowed", name),
                    is_error: true,
                }));
                self.emit(AgentEvent::ToolResult {
                    id: id.clone(),
                    name: name.clone(),
                    result: "Permission denied".to_string(),
                    is_error: true,
                })
                .await?;
            } else {
                approved.push((id.clone(), name.clone(), input.clone()));
            }
        }

        // Phase 2: Parallel execution of approved tools
        let mut indexed_results: Vec<(usize, ContentBlock)> = Vec::new();

        // Add denied results with their original indices
        indexed_results.extend(denied_results);

        if !approved.is_empty() {
            // Capture shared state for spawned tasks
            let kernel = Arc::clone(&self.params.kernel);
            let tool_ctx = self.tool_ctx.clone();
            let mode = self.params.mode;
            let event_tx = self.params.event_tx.clone();

            let mut join_set = tokio::task::JoinSet::new();

            for (id, name, input) in approved {
                let kernel = Arc::clone(&kernel);
                let tool_ctx = tool_ctx.clone();
                let event_tx = event_tx.clone();

                // Find the original index for ordering
                let original_idx = tool_uses
                    .iter()
                    .position(|(tid, _, _)| tid == &id)
                    .unwrap_or(0);

                join_set.spawn(async move {
                    let tool_start = Instant::now();
                    let result = execute_tool(&kernel, &name, input, &tool_ctx, &mode).await;
                    let tool_duration = tool_start.elapsed();

                    let (content_block, tool_result_event) = match result {
                        Ok(result) => {
                            info!(
                                tool = name.as_str(),
                                duration_ms = tool_duration.as_millis() as u64,
                                ok = !result.is_error,
                                output_len = result.content.len(),
                                "tool exec (parallel)"
                            );
                            let event = AgentEvent::ToolResult {
                                id: id.clone(),
                                name: name.clone(),
                                result: result.content.clone(),
                                is_error: result.is_error,
                            };
                            let block = ContentBlock::ToolResult {
                                tool_use_id: id,
                                content: result.content,
                                is_error: result.is_error,
                            };
                            (block, event)
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            info!(
                                tool = name.as_str(),
                                duration_ms = tool_duration.as_millis() as u64,
                                ok = false,
                                error = %err_msg,
                                "tool exec (parallel)"
                            );
                            let event = AgentEvent::ToolResult {
                                id: id.clone(),
                                name: name.clone(),
                                result: err_msg.clone(),
                                is_error: true,
                            };
                            let block = ContentBlock::ToolResult {
                                tool_use_id: id,
                                content: err_msg,
                                is_error: true,
                            };
                            (block, event)
                        }
                    };

                    // Emit event (best-effort, task may outlive TUI)
                    let _ = event_tx.send(tool_result_event).await;

                    (original_idx, content_block)
                });
            }

            // Collect results from JoinSet
            while let Some(join_result) = join_set.join_next().await {
                match join_result {
                    Ok((idx, block)) => {
                        indexed_results.push((idx, block));
                    }
                    Err(e) => {
                        error!(error = %e, "tool task panicked");
                    }
                }
            }
        }

        // Sort results by original tool_use index to maintain order
        indexed_results.sort_by_key(|(idx, _)| *idx);
        let tool_result_blocks: Vec<ContentBlock> = indexed_results
            .into_iter()
            .map(|(_, block)| block)
            .collect();

        // Add tool results as a user message (per Anthropic API convention)
        let tool_results_msg = Message {
            role: MessageRole::User,
            content: tool_result_blocks,
        };
        if let Err(e) = self.params.session_manager.save_message(&self.params.session.id, &tool_results_msg) {
            error!(error = %e, "failed to persist message");
        }
        self.params.messages.push(tool_results_msg);

        Ok(())
    }
}
