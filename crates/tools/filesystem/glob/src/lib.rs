use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{GlobOptions, PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

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
            Some(p) => Some(
                ctx.backend
                    .resolve_path(p, false)
                    .map_err(|e| {
                        LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(e.to_string()))
                    })?
                    .to_string_lossy()
                    .into_owned(),
            ),
            None => None,
        };

        let limit = input["limit"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_LIMIT);
        let offset = input["offset"].as_u64().map(|n| n as usize).unwrap_or(0);

        let opts = GlobOptions {
            pattern: pattern.to_string(),
            path: search_path,
            type_filter: input["type"].as_str().map(String::from),
            max_results: 10_000,
        };

        let result = ctx.backend.glob(&opts).await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(e.to_string()))
        })?;

        // Sort by modification time, newest first.
        let mut entries = result.entries;
        entries.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));

        let total_found = entries.len();
        if total_found == 0 {
            return Ok(ToolResult::success("No files matched the pattern."));
        }

        let page: Vec<&str> = entries
            .iter()
            .skip(offset)
            .take(limit)
            .map(|e| e.path.as_str())
            .collect();
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
