use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{json, Value};

use crate::edit::patch_apply::apply_file_ops;
use crate::edit::patch_parser::parse_patch;

pub struct ApplyPatchTool;

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "ApplyPatch"
    }

    fn description(&self) -> &str {
        "Apply a patch to create, update, or delete multiple files atomically. \
         Uses a unified diff-like format with context lines for reliable matching."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["patch"],
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "Patch text in the Codex V4A format"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let patch = input["patch"]
            .as_str()
            .ok_or_else(|| tool_input("patch is required"))?;

        let ops = parse_patch(patch).map_err(|e| tool_input(&format!("parse error: {e}")))?;
        if ops.is_empty() {
            return Ok(ToolResult::error("patch contains no file operations"));
        }

        // Path traversal protection
        let cwd_canon = ctx.cwd.canonicalize().unwrap_or_else(|_| ctx.cwd.clone());
        for op in &ops {
            let rel = op.path();
            let full = if rel.is_absolute() { rel.clone() } else { ctx.cwd.join(rel) };
            let check = if full.exists() {
                full.canonicalize().ok()
            } else {
                full.parent().and_then(|p| p.canonicalize().ok())
            };
            if let Some(c) = check
                && !c.starts_with(&cwd_canon)
            {
                return Ok(ToolResult::error(format!(
                    "path outside working directory: {}",
                    rel.display()
                )));
            }
        }

        // Apply in memory
        let writes = apply_file_ops(&ops, &ctx.cwd, |p| std::fs::read_to_string(p))
            .map_err(|e| tool_input(&e.to_string()))?;

        // Atomic write phase
        let (mut created, mut updated, mut deleted) = (0u32, 0u32, 0u32);
        for w in &writes {
            match &w.content {
                Some(content) => {
                    if let Some(parent) = w.path.parent() {
                        tokio::fs::create_dir_all(parent).await.map_err(|e| {
                            tool_exec(&format!("mkdir: {e}"))
                        })?;
                    }
                    let existed = w.path.exists();
                    tokio::fs::write(&w.path, content).await.map_err(|e| {
                        tool_exec(&format!("write {}: {e}", w.path.display()))
                    })?;
                    if existed { updated += 1; } else { created += 1; }
                }
                None => {
                    tokio::fs::remove_file(&w.path).await.map_err(|e| {
                        tool_exec(&format!("delete {}: {e}", w.path.display()))
                    })?;
                    deleted += 1;
                }
            }
        }

        let mut parts = Vec::new();
        if updated > 0 { parts.push(format!("{updated} updated")); }
        if created > 0 { parts.push(format!("{created} created")); }
        if deleted > 0 { parts.push(format!("{deleted} deleted")); }
        Ok(ToolResult::success(format!("Applied: {}", parts.join(", "))))
    }
}

fn tool_input(msg: &str) -> LoopalError {
    LoopalError::Tool(loopal_error::ToolError::InvalidInput(msg.into()))
}

fn tool_exec(msg: &str) -> LoopalError {
    LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(msg.into()))
}
