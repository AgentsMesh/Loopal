//! Collect parallel tool results from a JoinSet, racing against cancellation.

use std::collections::HashSet;
use std::sync::Arc;

use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use tracing::{error, info};

use crate::frontend::traits::AgentFrontend;

use super::cancel::TurnCancel;

/// Collect results from JoinSet, racing against cancellation.
pub(super) async fn collect_results(
    join_set: &mut tokio::task::JoinSet<(usize, ContentBlock)>,
    approved: &[(String, String, serde_json::Value)],
    tool_uses: &[(String, String, serde_json::Value)],
    frontend: &Arc<dyn AgentFrontend>,
    cancel: &TurnCancel,
) -> Vec<(usize, ContentBlock)> {
    let mut results = Vec::new();
    let mut collected_ids: HashSet<String> = HashSet::new();

    loop {
        if cancel.is_cancelled() {
            info!("cancelled before collecting, aborting remaining tools");
            join_set.abort_all();
            break;
        }
        tokio::select! {
            biased;
            join_result = join_set.join_next() => {
                let Some(join_result) = join_result else { break; };
                match join_result {
                    Ok((idx, block)) => {
                        if let ContentBlock::ToolResult { ref tool_use_id, .. } = block {
                            collected_ids.insert(tool_use_id.clone());
                        }
                        results.push((idx, block));
                    }
                    Err(e) if e.is_cancelled() => {}
                    Err(e) => error!(error = %e, "tool task panicked"),
                }
            }
            _ = cancel.cancelled() => {
                info!("cancelled during tool execution, aborting remaining tools");
                join_set.abort_all();
                while let Some(join_result) = join_set.join_next().await {
                    if let Ok((idx, block)) = join_result {
                        if let ContentBlock::ToolResult { ref tool_use_id, .. } = block {
                            collected_ids.insert(tool_use_id.clone());
                        }
                        results.push((idx, block));
                    }
                }
                break;
            }
        }
    }

    // Synthesise "Interrupted by user" for tools that were not collected
    let emitter = frontend.event_emitter();
    for (id, name, _) in approved {
        if collected_ids.contains(id) {
            continue;
        }
        let orig_idx = tool_uses
            .iter()
            .position(|(tid, _, _)| tid == id)
            .unwrap_or(0);
        let _ = emitter
            .emit(AgentEventPayload::ToolResult {
                id: id.clone(),
                name: name.clone(),
                result: "Interrupted by user".into(),
                is_error: true,
                duration_ms: None,
                is_completion: false,
                metadata: None,
            })
            .await;
        results.push((
            orig_idx,
            ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: "Interrupted by user".into(),
                is_error: true,
                is_completion: false,
                metadata: None,
            },
        ));
    }

    results
}
