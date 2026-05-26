mod helpers;

use loopal_context::compact_config::{
    REHYDRATE_PER_FILE_BYTES, REHYDRATE_TIMEOUT, REHYDRATE_TOP_N, REHYDRATE_TOTAL_BYTES,
};
use loopal_context::middleware::touched_files::TouchedFile;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::MessageOrigin;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_turn::{CompactionRehydrate, TurnStep};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use self::helpers::{collect_rehydrated_files, trim_body};
use super::runner::AgentLoopRunner;

#[derive(Debug, Default)]
pub struct RehydrateStats {
    pub files_attempted: usize,
    pub files_succeeded: usize,
    pub bytes_injected: usize,
    pub cancelled: bool,
}

impl AgentLoopRunner {
    pub async fn compact_rehydrate(
        &mut self,
        touched: &[TouchedFile],
        cancel: &CancellationToken,
    ) -> RehydrateStats {
        let mut stats = RehydrateStats::default();
        if touched.is_empty() {
            return stats;
        }
        if cancel.is_cancelled() {
            stats.cancelled = true;
            return stats;
        }
        let Some(read_tool) = self.params.deps.kernel.get_tool("Read") else {
            warn!("Read tool not registered; skipping rehydrate");
            return stats;
        };

        let selected: Vec<&TouchedFile> = touched.iter().take(REHYDRATE_TOP_N).collect();
        stats.files_attempted = selected.len();

        let read_futs = selected.iter().map(|tf| async {
            let input = serde_json::json!({ "file_path": tf.path });
            let outcome = timeout(
                REHYDRATE_TIMEOUT,
                read_tool.execute(input.clone(), &self.tool_ctx),
            )
            .await;
            (tf.path.clone(), input, outcome)
        });

        // Dropping read_futs on cancel aborts in-flight Reads — no message
        // is persisted, so no orphan ToolUse can survive.
        let outcomes = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                warn!("rehydrate cancelled before reads completed");
                stats.cancelled = true;
                return stats;
            }
            result = futures::future::join_all(read_futs) => result,
        };

        let mut tool_uses: Vec<ContentBlock> = Vec::new();
        let mut tool_results: Vec<ContentBlock> = Vec::new();
        let mut budget_remaining = REHYDRATE_TOTAL_BYTES;

        for (path, input, outcome) in outcomes {
            if budget_remaining == 0 {
                break;
            }
            let result = match outcome {
                Ok(Ok(r)) if !r.is_error => r,
                Ok(Ok(r)) => {
                    warn!(path = %path, error = %r.content, "rehydrate read returned tool error");
                    continue;
                }
                Ok(Err(e)) => {
                    warn!(path = %path, error = %e, "rehydrate read failed");
                    continue;
                }
                Err(_) => {
                    warn!(path = %path, "rehydrate read timed out");
                    continue;
                }
            };
            let body = trim_body(result, REHYDRATE_PER_FILE_BYTES.min(budget_remaining));
            budget_remaining = budget_remaining.saturating_sub(body.len());
            stats.bytes_injected += body.len();
            stats.files_succeeded += 1;

            let call_id = format!("compact-rehydrate-{}", uuid::Uuid::new_v4());
            tool_uses.push(ContentBlock::ToolUse {
                id: call_id.clone(),
                name: "Read".to_string(),
                input,
            });
            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: call_id,
                content: body,
                images: Vec::new(),
                is_error: false,
                metadata: None,
            });
        }

        if tool_uses.is_empty() {
            return stats;
        }

        // Last cancel check before the persist critical section — no .await
        // between the two save_message calls, so the persist block is atomic.
        if cancel.is_cancelled() {
            warn!("rehydrate cancelled before persist; discarding read results");
            stats.cancelled = true;
            stats.files_succeeded = 0;
            stats.bytes_injected = 0;
            return stats;
        }

        let assistant = Message {
            id: None,
            role: MessageRole::Assistant,
            content: tool_uses,
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        };
        // Partial failure: model can't tell which files dropped out without
        // an explicit note — append text block so it re-Reads on demand.
        if stats.files_succeeded < stats.files_attempted {
            let dropped = stats.files_attempted - stats.files_succeeded;
            tool_results.push(ContentBlock::Text {
                text: format!(
                    "[rehydrate partial: {dropped} of {attempted} touched files were not \
                     re-read (read error / timeout / over budget). Re-Read them on demand \
                     before editing.]",
                    attempted = stats.files_attempted,
                ),
            });
        }
        let user = Message {
            id: None,
            role: MessageRole::User,
            content: tool_results,
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        };

        let rehydrated_files = collect_rehydrated_files(&assistant.content, &user.content);
        if !rehydrated_files.is_empty()
            && let Err(e) =
                self.append_step_record(TurnStep::CompactionRehydrate(CompactionRehydrate {
                    files: rehydrated_files,
                }))
        {
            warn!(error = %e, "append_step(CompactionRehydrate) failed");
        }

        info!(
            files_attempted = stats.files_attempted,
            files_succeeded = stats.files_succeeded,
            bytes_injected = stats.bytes_injected,
            "post-compact rehydrate complete"
        );
        let _ = self
            .emit(AgentEventPayload::Stream {
                text: format!(
                    "[rehydrated {} files, {} bytes]\n",
                    stats.files_succeeded, stats.bytes_injected
                ),
            })
            .await;
        stats
    }
}

#[cfg(test)]
mod tests;
