use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{LsEntry, PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use crate::ls_format;

pub struct LsTool;

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "Ls"
    }

    fn description(&self) -> &str {
        "List directory contents. Use long mode for size, permissions, and mtime. \
         When path points to a file, shows detailed file info (stat)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory or file path (default: cwd)"
                },
                "long": {
                    "type": "boolean",
                    "description": "Show permissions, size, and modification time (default: false)"
                },
                "all": {
                    "type": "boolean",
                    "description": "Include hidden files starting with '.' (default: false)"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let raw_path = input["path"].as_str().unwrap_or(".");

        // Resolve path via backend (handles sandbox policy)
        let target = match ctx.backend.resolve_path(raw_path, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        // Single file -> stat-like output via backend
        let info = match ctx.backend.file_info(target.to_str().unwrap_or(".")).await {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        if !info.is_dir {
            return Ok(ToolResult::success(ls_format::format_stat_from_info(
                &target, &info,
            )));
        }

        // Directory listing via backend
        let long = input["long"].as_bool().unwrap_or(false);
        let show_all = input["all"].as_bool().unwrap_or(false);

        let ls_result = match ctx.backend.ls(target.to_str().unwrap_or(".")).await {
            Ok(r) => r,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        format_entries(&ls_result.entries, long, show_all)
    }
}

fn format_entries(
    entries: &[LsEntry],
    long: bool,
    show_all: bool,
) -> Result<ToolResult, LoopalError> {
    let filtered: Vec<&LsEntry> = entries
        .iter()
        .filter(|e| show_all || !e.name.starts_with('.'))
        .collect();

    if filtered.is_empty() {
        return Ok(ToolResult::success("(empty directory)"));
    }

    let lines: Vec<String> = filtered
        .iter()
        .map(|e| {
            let indicator = if e.is_dir {
                "/"
            } else if e.is_symlink {
                "@"
            } else {
                ""
            };
            if long {
                ls_format::format_long_from_entry(e, indicator)
            } else {
                format!("{}{indicator}", e.name)
            }
        })
        .collect();

    Ok(ToolResult::success(lines.join("\n")))
}
