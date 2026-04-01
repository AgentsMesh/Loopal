//! Shared helpers for collaboration tools (Agent, SendMessage).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopal_error::LoopalError;
use loopal_tool_api::ToolContext;

use crate::shared::AgentShared;

/// Extract `AgentShared` from `ToolContext.shared`.
pub(crate) fn extract_shared(ctx: &ToolContext) -> Result<Arc<AgentShared>, LoopalError> {
    ctx.shared
        .as_ref()
        .and_then(|s| s.downcast_ref::<Arc<AgentShared>>())
        .cloned()
        .ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "AgentShared not available".into(),
            ))
        })
}

pub(crate) fn require_str<'a>(
    input: &'a serde_json::Value,
    field: &str,
) -> Result<&'a str, LoopalError> {
    input.get(field).and_then(|v| v.as_str()).ok_or_else(|| {
        LoopalError::Tool(loopal_error::ToolError::InvalidInput(format!(
            "missing '{field}'"
        )))
    })
}

pub(crate) fn create_agent_worktree(
    cwd: &Path,
    agent_name: &str,
    uid: &str,
) -> Result<(loopal_git::WorktreeInfo, PathBuf), LoopalError> {
    let root = loopal_git::repo_root(cwd)
        .ok_or_else(|| LoopalError::Other("Not a git repository".into()))?;
    let wt_name = format!("agent-{agent_name}-{uid}");
    let info = loopal_git::create_worktree(&root, &wt_name)
        .map_err(|e| LoopalError::Other(format!("worktree: {e}")))?;
    Ok((info, root))
}
