use async_trait::async_trait;
use globset::{Glob, GlobSetBuilder};
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::path::PathBuf;
use walkdir::WalkDir;

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
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopAgentError> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                    "pattern is required".into(),
                ))
            })?;

        let search_path = match input["path"].as_str() {
            Some(p) => {
                let pb = PathBuf::from(p);
                if pb.is_absolute() { pb } else { ctx.cwd.join(pb) }
            }
            None => ctx.cwd.clone(),
        };

        let limit = input["limit"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_LIMIT);
        let offset = input["offset"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(0);

        let glob = Glob::new(pattern).map_err(|e| {
            LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                format!("Invalid glob pattern: {}", e),
            ))
        })?;

        let mut builder = GlobSetBuilder::new();
        builder.add(glob);
        let glob_set = builder.build().map_err(|e| {
            LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                format!("Failed to build glob set: {}", e),
            ))
        })?;

        let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in WalkDir::new(&search_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if let Ok(rel) = path.strip_prefix(&search_path)
                && glob_set.is_match(rel)
            {
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
            output.push_str(&format!(
                "\n\n(Use offset={} to see more.)", page_end
            ));
        }

        Ok(ToolResult::success(output))
    }
}
