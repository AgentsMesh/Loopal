use std::collections::HashSet;

use tracing::{error, info};

use super::cancel::TurnCancel;
use super::tool_result_sink::PendingToolResult;

pub(super) async fn collect_results(
    join_set: &mut tokio::task::JoinSet<(usize, PendingToolResult)>,
    approved: &[(String, String)],
    tool_uses: &[(String, String, serde_json::Value)],
    cancel: &TurnCancel,
) -> Vec<(usize, PendingToolResult)> {
    let mut results = Vec::new();
    let mut collected_ids = HashSet::new();

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
                    Ok((index, result)) => {
                        collected_ids.insert(result.id().to_string());
                        results.push((index, result));
                    }
                    Err(error) if error.is_cancelled() => {}
                    Err(error) => error!(%error, "tool task panicked"),
                }
            }
            _ = cancel.cancelled() => {
                info!("cancelled during tool execution, aborting remaining tools");
                join_set.abort_all();
                while let Some(join_result) = join_set.join_next().await {
                    if let Ok((index, result)) = join_result {
                        collected_ids.insert(result.id().to_string());
                        results.push((index, result));
                    }
                }
                break;
            }
        }
    }

    for (id, name) in approved {
        let index = tool_uses
            .iter()
            .position(|(tool_id, _, _)| tool_id == id)
            .unwrap_or(0);
        if collected_ids.contains(id) {
            continue;
        }
        results.push((index, interrupted(id, name)));
    }
    results
}

fn interrupted(id: &str, name: &str) -> PendingToolResult {
    PendingToolResult::new(
        id,
        name,
        "Interrupted by user",
        true,
        Some(loopal_tool_invocation::ToolResultMetadata::cancelled(
            loopal_tool_invocation::CancelCause::UserInterrupt,
        )),
    )
}

#[cfg(test)]
#[path = "tool_collect/tests.rs"]
mod tests;
