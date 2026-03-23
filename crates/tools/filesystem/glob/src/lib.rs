use async_trait::async_trait;
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};
use std::path::PathBuf;

use loopal_tool_grep::grep_search::type_to_extensions;

pub struct GlobTool;

/// Default maximum number of results returned per page.
const DEFAULT_LIMIT: usize = 100;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns paths sorted by modification time (newest first). \
         Use offset for pagination when results exceed the limit."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g. \"**/*.rs\", \"src/**/*.ts\")"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: cwd)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results per page (default: 100)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Number of results to skip for pagination (default: 0)"
                },
                "type": {
                    "type": "string",
                    "description": "File type filter (e.g. js, py, rust, go)"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let pattern = input["pattern"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "pattern is required".into(),
            ))
        })?;

        let search_path = match input["path"].as_str() {
            Some(p) => match ctx.backend.resolve_path(p, false) {
                Ok(resolved) => resolved,
                Err(e) => return Ok(ToolResult::error(e.to_string())),
            },
            None => ctx.backend.cwd().to_path_buf(),
        };

        let limit = input["limit"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_LIMIT);
        let offset = input["offset"].as_u64().map(|n| n as usize).unwrap_or(0);

        let glob = Glob::new(pattern).map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(format!(
                "Invalid glob pattern: {e}"
            )))
        })?;

        let mut builder = GlobSetBuilder::new();
        builder.add(glob);
        let glob_set = builder.build().map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(format!(
                "Failed to build glob set: {e}"
            )))
        })?;

        let type_exts: Option<Vec<&str>> = input["type"].as_str().map(|t| {
            type_to_extensions(t).unwrap_or_default() // unknown type → empty → no matches
        });

        let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in WalkBuilder::new(&search_path)
            .follow_links(true)
            .build()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Ok(rel) = path.strip_prefix(&search_path)
                && glob_set.is_match(rel)
            {
                // Apply type extension filter
                if let Some(ref exts) = type_exts {
                    let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if !exts.contains(&file_ext) {
                        continue;
                    }
                }
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::UNIX_EPOCH);
                matches.push((path.to_path_buf(), mtime));
            }
        }

        // Sort by modification time, newest first
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        let total_found = matches.len();
        let page: Vec<String> = matches
            .iter()
            .skip(offset)
            .take(limit)
            .map(|(p, _)| p.display().to_string())
            .collect();

        if total_found == 0 {
            return Ok(ToolResult::success("No files matched the pattern."));
        }

        let page_end = (offset + page.len()).min(total_found);
        let mut output = format!(
            "Found {} files. Showing {}-{}:\n{}",
            total_found,
            offset + 1,
            page_end,
            page.join("\n")
        );

        if page_end < total_found {
            output.push_str(&format!("\n\n(Use offset={page_end} to see more.)"));
        }

        Ok(ToolResult::success(output))
    }
}
