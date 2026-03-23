use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::PermissionLevel;
use loopal_tool_api::{Tool, ToolContext, ToolResult};

use crate::shared::{AgentShared, WorktreeState};
use crate::tools::agent::extract_shared;

// ---------------------------------------------------------------------------
// EnterWorktree
// ---------------------------------------------------------------------------

pub struct EnterWorktreeTool;

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &str {
        "EnterWorktree"
    }
    fn description(&self) -> &str {
        "Create a git worktree and switch the session's working directory into it."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Worktree name (auto-generated if omitted)" }
            }
        })
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        if shared.worktree_state.lock().await.is_some() {
            return Ok(ToolResult::error(
                "Already in a worktree. Call ExitWorktree first.",
            ));
        }
        let repo_root = match loopal_git::repo_root(&shared.cwd) {
            Some(r) => r,
            None => return Ok(ToolResult::error("Not inside a git repository.")),
        };
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("wt-{}", &uuid::Uuid::new_v4().to_string()[..8]));

        let info = match loopal_git::create_worktree(&repo_root, &name) {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(format!("Failed: {e}"))),
        };

        // Signal the runner to switch cwd after this tool batch completes
        if let Ok(mut guard) = ctx.pending_cwd_switch.lock() {
            *guard = Some(info.path.clone());
        }
        *shared.worktree_state.lock().await = Some(WorktreeState {
            original_cwd: shared.cwd.clone(),
            worktree_path: info.path.clone(),
            worktree_name: info.name.clone(),
            repo_root,
        });

        Ok(ToolResult::success(format!(
            "Worktree '{}' created at {}.\nBranch: {}\nWorking directory switched.",
            info.name,
            info.path.display(),
            info.branch,
        )))
    }
}

// ---------------------------------------------------------------------------
// ExitWorktree
// ---------------------------------------------------------------------------

pub struct ExitWorktreeTool;

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &str {
        "ExitWorktree"
    }
    fn description(&self) -> &str {
        "Exit the current worktree session and return to the original directory."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string", "enum": ["keep", "remove"],
                    "description": "\"keep\" preserves worktree; \"remove\" deletes it."
                },
                "discard_changes": {
                    "type": "boolean",
                    "description": "Force remove with uncommitted changes. Default: false."
                }
            },
            "required": ["action"]
        })
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        execute_exit(&shared, &input, ctx).await
    }
}

async fn execute_exit(
    shared: &Arc<AgentShared>,
    input: &serde_json::Value,
    ctx: &ToolContext,
) -> Result<ToolResult, LoopalError> {
    let state = shared.worktree_state.lock().await.take();
    let state = match state {
        Some(s) => s,
        None => return Ok(ToolResult::success("No active worktree session.")),
    };

    let action = input
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("keep");

    if action == "remove" {
        let force = input
            .get("discard_changes")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !force {
            match loopal_git::worktree_has_changes(&state.worktree_path) {
                Ok(true) => {
                    *shared.worktree_state.lock().await = Some(state);
                    return Ok(ToolResult::error(
                        "Worktree has uncommitted changes. Set discard_changes=true to force.",
                    ));
                }
                Err(e) => {
                    *shared.worktree_state.lock().await = Some(state);
                    return Ok(ToolResult::error(format!(
                        "Cannot check worktree status: {e}"
                    )));
                }
                Ok(false) => {} // clean — proceed with removal
            }
        }
        if let Err(e) = loopal_git::remove_worktree(&state.repo_root, &state.worktree_name, force) {
            *shared.worktree_state.lock().await = Some(state);
            return Ok(ToolResult::error(format!("Failed to remove: {e}")));
        }
    }

    // Only signal cwd switch on the success path (after validation and removal)
    if let Ok(mut guard) = ctx.pending_cwd_switch.lock() {
        *guard = Some(state.original_cwd.clone());
    }

    if action == "remove" {
        Ok(ToolResult::success(format!(
            "Worktree '{}' removed. Restored to {}.",
            state.worktree_name,
            state.original_cwd.display(),
        )))
    } else {
        Ok(ToolResult::success(format!(
            "Worktree '{}' kept at {}. Restored to {}.",
            state.worktree_name,
            state.worktree_path.display(),
            state.original_cwd.display(),
        )))
    }
}
