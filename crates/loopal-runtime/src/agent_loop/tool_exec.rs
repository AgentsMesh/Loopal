use std::sync::Arc;
use std::time::Instant;

use loopal_kernel::Kernel;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::{OutputTail, ToolContext, ToolResult};
use tracing::Instrument;

use crate::frontend::traits::AgentFrontend;
use crate::mode::AgentMode;
use crate::tool_action::PreparedToolAction;
use crate::tool_execution_output::ToolExecutionOutput;
use crate::tool_pipeline::execute_tool_output;

use super::cancel::TurnCancel;
use super::tool_collect::collect_results;
use super::tool_progress::maybe_spawn_progress;
use super::tool_result_sink::PendingToolResult;

pub(crate) async fn execute_tool_watchdogged(
    kernel: &Kernel,
    action: PreparedToolAction,
    ctx: &ToolContext,
    mode: &AgentMode,
) -> loopal_error::Result<ToolExecutionOutput> {
    let deadline =
        super::tool_watchdog::watchdog_deadline(action.tool_name(), action.placeholder_input());
    let Some(deadline) = deadline else {
        return execute_tool_output(kernel, action, ctx, mode).await;
    };
    match tokio::time::timeout(deadline, execute_tool_output(kernel, action, ctx, mode)).await {
        Ok(result) => result,
        Err(_) => Ok(ToolExecutionOutput::unseeded(
            super::tool_watchdog::timeout_result(deadline),
        )),
    }
}

pub(super) async fn execute_approved_tools(
    approved: Vec<PreparedToolAction>,
    tool_uses: &[(String, String, serde_json::Value)],
    kernel: Arc<Kernel>,
    tool_ctx: ToolContext,
    mode: AgentMode,
    frontend: &Arc<dyn AgentFrontend>,
    cancel: &TurnCancel,
) -> Vec<(usize, PendingToolResult)> {
    let interrupted = approved
        .iter()
        .map(|action| (action.id().to_string(), action.tool_name().to_string()))
        .collect::<Vec<_>>();
    let mut join_set = tokio::task::JoinSet::new();
    let parent_span = tracing::Span::current();

    for action in approved {
        let kernel = Arc::clone(&kernel);
        let mut tool_ctx = tool_ctx.clone();
        let emitter = frontend.event_emitter();
        let progress_emitter = frontend.event_emitter();
        let span = parent_span.clone();
        let id = action.id().to_string();
        let name = action.tool_name().to_string();
        let input = action.placeholder_input().clone();
        let original_index = tool_uses
            .iter()
            .position(|(tool_id, _, _)| tool_id == &id)
            .unwrap_or(0);
        let tail = if name == "Bash" {
            let tail = Arc::new(OutputTail::new(5));
            tool_ctx.output_tail = Some(Arc::clone(&tail));
            Some(tail)
        } else {
            None
        };

        join_set.spawn(
            loopal_protocol::event_id::propagate_to_spawn(async move {
                emitter
                    .emit_best_effort(
                        AgentEventPayload::ToolProgress {
                            id: id.clone(),
                            name: name.clone(),
                            output_tail: String::new(),
                            elapsed_ms: 0,
                        },
                        "agent_loop::tool_exec::tool_started",
                    )
                    .await;
                let progress =
                    maybe_spawn_progress(&name, &input, id.clone(), progress_emitter, tail);
                let start = Instant::now();
                let result = execute_tool_watchdogged(&kernel, action, &tool_ctx, &mode).await;
                if let Some(progress) = progress {
                    progress.abort();
                }
                (
                    original_index,
                    pending_execution_result(id, name, start.elapsed(), result),
                )
            })
            .instrument(span),
        );
    }

    collect_results(&mut join_set, &interrupted, tool_uses, cancel).await
}

fn pending_execution_result(
    id: String,
    name: String,
    duration: std::time::Duration,
    output: loopal_error::Result<ToolExecutionOutput>,
) -> PendingToolResult {
    match output {
        Ok(output) => {
            let result = output
                .outcome
                .unwrap_or_else(|error| ToolResult::error(error.to_string()));
            PendingToolResult::from_result(id, name, result)
                .with_duration(duration)
                .with_guard(output.seed, output.image_policy)
        }
        Err(error) => {
            PendingToolResult::from_result(id, name, ToolResult::error(error.to_string()))
                .with_duration(duration)
        }
    }
}
