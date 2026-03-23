use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use loopal_edit_core::patch_apply::apply_file_ops;
use loopal_edit_core::patch_parser::parse_patch;

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

        // Path validation via backend (write mode)
        for op in &ops {
            let rel = op.path();
            let path_str = rel.to_string_lossy();
            if let Err(e) = ctx.backend.resolve_path(&path_str, true) {
                return Ok(ToolResult::error(e.to_string()));
            }
        }

        // Apply in memory — read files using std::fs on backend-resolved paths
        let cwd = ctx.backend.cwd().to_path_buf();
        let writes = apply_file_ops(&ops, &cwd, |p| std::fs::read_to_string(p))
            .map_err(|e| tool_input(&e.to_string()))?;

        // Write phase via backend
        let (mut created, mut updated, mut deleted) = (0u32, 0u32, 0u32);
        for w in &writes {
            let path_str = w.path.to_string_lossy();
            match &w.content {
                Some(content) => {
                    let existed = w.path.exists();
                    match ctx.backend.write(&path_str, content).await {
                        Ok(_) => {
                            if existed {
                                updated += 1;
                            } else {
                                created += 1;
                            }
                        }
                        Err(e) => {
                            return Ok(ToolResult::error(format!(
                                "write {}: {e}",
                                w.path.display()
                            )));
                        }
                    }
                }
                None => match ctx.backend.remove(&path_str).await {
                    Ok(()) => {
                        deleted += 1;
                    }
                    Err(e) => {
                        return Ok(ToolResult::error(format!(
                            "delete {}: {e}",
                            w.path.display()
                        )));
                    }
                },
            }
        }

        let mut parts = Vec::new();
        if updated > 0 {
            parts.push(format!("{updated} updated"));
        }
        if created > 0 {
            parts.push(format!("{created} created"));
        }
        if deleted > 0 {
            parts.push(format!("{deleted} deleted"));
        }
        Ok(ToolResult::success(format!(
            "Applied: {}",
            parts.join(", ")
        )))
    }
}

fn tool_input(msg: &str) -> LoopalError {
    LoopalError::Tool(loopal_error::ToolError::InvalidInput(msg.into()))
}
