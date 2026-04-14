//! Fork context + background cleanup helpers for the Agent tool.

use std::sync::Arc;

use crate::shared::AgentShared;
use crate::spawn::wait_agent;

pub(super) fn build_fork_context(shared: &AgentShared) -> Option<Vec<loopal_message::Message>> {
    let snapshot = shared
        .message_snapshot
        .read()
        .unwrap_or_else(|e| e.into_inner());
    if snapshot.is_empty() {
        None
    } else {
        Some(loopal_context::fork::compress_for_fork(&snapshot))
    }
}

pub(super) fn spawn_bg_cleanup(
    shared: Arc<AgentShared>,
    name: String,
    wt: Option<(loopal_git::WorktreeInfo, std::path::PathBuf)>,
) {
    if let Some((info, root)) = wt {
        tokio::spawn(async move {
            let timeout = std::time::Duration::from_secs(3600);
            match tokio::time::timeout(timeout, wait_agent(&shared, &name)).await {
                Ok(_) => {
                    loopal_git::cleanup_if_clean(&root, &info);
                }
                Err(_) => tracing::warn!(agent = %name, "background agent timed out"),
            }
        });
    }
}
