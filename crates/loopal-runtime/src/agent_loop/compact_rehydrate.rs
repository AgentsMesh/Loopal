use loopal_context::compact_config::{
    REHYDRATE_PER_FILE_BYTES, REHYDRATE_TIMEOUT, REHYDRATE_TOP_N, REHYDRATE_TOTAL_BYTES,
};
use loopal_context::middleware::touched_files::TouchedFile;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ContentBlock, Message, MessageRole};
use loopal_tool_api::ToolResult;
use loopal_turn::MessageOrigin;
use loopal_turn::{CompactionRehydrate, RehydratedFile, ToolCallId, TurnStep};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

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

        // Reads are IO-bound and independent — fan them out in parallel.
        // `tool_ctx` is shared by `&` so multiple in-flight Reads is safe.
        let read_futs = selected.iter().map(|tf| async {
            let input = serde_json::json!({ "file_path": tf.path });
            let outcome = timeout(
                REHYDRATE_TIMEOUT,
                read_tool.execute(input.clone(), &self.tool_ctx),
            )
            .await;
            (tf.path.clone(), input, outcome)
        });

        // Cancel must abort before any message is persisted. Dropping
        // `read_futs` here cancels the in-flight Reads — they never
        // contribute to the conversation, so no orphan ToolUse can be
        // saved.
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

        // Last cancel check before the persist critical section. After the
        // first `save_message`, the only safe option is to also save the
        // second (no `.await` between them, so no further cancellation
        // point — the persist block runs to completion or not at all).
        if cancel.is_cancelled() {
            warn!("rehydrate cancelled before persist; discarding read results");
            stats.cancelled = true;
            stats.files_succeeded = 0;
            stats.bytes_injected = 0;
            return stats;
        }

        let mut assistant = Message {
            id: None,
            role: MessageRole::Assistant,
            content: tool_uses,
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        };
        // Partial-failure path: model only sees the ToolResults that
        // succeeded; without an explicit note it can't tell which files
        // dropped out. Append a text block to the same user message so
        // the model knows to re-Read them on demand.
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
        let mut user = Message {
            id: None,
            role: MessageRole::User,
            content: tool_results,
            origin: Some(MessageOrigin::CompactionRehydrate),
            ephemeral_in_history: false,
        };

        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut assistant)
        {
            warn!(error = %e, "rehydrate assistant persist failed");
            return stats;
        }
        if let Err(e) = self
            .params
            .deps
            .session_manager
            .save_message(&self.params.session.id, &mut user)
        {
            warn!(error = %e, "rehydrate tool_result persist failed");
            return stats;
        }

        // Domain mirror: emit a CompactionRehydrate step carrying the rehydrated files.
        let rehydrated_files: Vec<RehydratedFile> =
            collect_rehydrated_files(&assistant.content, &user.content);
        if !rehydrated_files.is_empty() {
            self.append_step_record(TurnStep::CompactionRehydrate(CompactionRehydrate {
                files: rehydrated_files,
            }));
        }

        self.params.store.push_assistant(assistant);
        self.params.store.push_tool_results(user);

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

fn collect_rehydrated_files(
    use_blocks: &[ContentBlock],
    result_blocks: &[ContentBlock],
) -> Vec<RehydratedFile> {
    let mut results: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for b in result_blocks {
        if let ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } = b
        {
            results.insert(tool_use_id.as_str(), content.as_str());
        }
    }
    use_blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, input, .. } => {
                let body = results.get(id.as_str())?;
                let path = input
                    .get("file_path")
                    .or_else(|| input.get("path"))
                    .and_then(|v| v.as_str())?;
                Some(RehydratedFile {
                    path: path.to_string(),
                    tool_call_id: ToolCallId::new(id),
                    content: body.to_string(),
                })
            }
            _ => None,
        })
        .collect()
}

fn trim_body(r: ToolResult, max_bytes: usize) -> String {
    if r.content.len() <= max_bytes {
        return r.content;
    }
    let mut end = max_bytes;
    while end > 0 && !r.content.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[...{} bytes truncated]",
        &r.content[..end],
        r.content.len() - end
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(s: &str) -> ToolResult {
        ToolResult {
            content: s.to_string(),
            images: Vec::new(),
            is_error: false,
            metadata: None,
        }
    }

    #[test]
    fn trim_body_passthrough_under_cap() {
        let out = trim_body(result("hello"), 1024);
        assert_eq!(out, "hello");
    }

    #[test]
    fn trim_body_truncates_with_marker() {
        let body = "x".repeat(100);
        let out = trim_body(result(&body), 10);
        assert!(out.starts_with(&"x".repeat(10)));
        assert!(out.contains("90 bytes truncated"));
    }

    #[test]
    fn trim_body_respects_char_boundary() {
        // 三个汉字 = 9 bytes UTF-8。max_bytes=4 落在汉字中间，会回退到 3.
        let out = trim_body(result("你好啊"), 4);
        assert!(out.starts_with("你"));
        assert!(!out.contains("好啊"));
    }

    #[test]
    fn rehydrate_stats_default_is_zero() {
        let s = RehydrateStats::default();
        assert_eq!(s.files_attempted, 0);
        assert_eq!(s.files_succeeded, 0);
        assert_eq!(s.bytes_injected, 0);
    }
}
