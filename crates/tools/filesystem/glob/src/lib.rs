use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{GlobOptions, PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct GlobTool;

#[derive(Deserialize, JsonSchema)]
pub struct GlobParams {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default, rename = "type")]
    pub type_filter: Option<String>,
}

const DEFAULT_LIMIT: usize = 100;

#[async_trait]
impl TypedTool<GlobParams> for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        "Fast file pattern matching tool that works with any codebase size.\n\
         - Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\".\n\
         - Returns matching file paths sorted by modification time.\n\
         - Use this tool when you need to find files by name patterns.\n\
         - When you are doing an open ended search that may require multiple rounds \
         of globbing and grepping, use the Agent tool instead."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        input: GlobParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let pattern = &input.pattern;

        let search_path = match input.path.as_deref() {
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

        let limit = input.limit.map(|n| n as usize).unwrap_or(DEFAULT_LIMIT);
        let offset = input.offset.map(|n| n as usize).unwrap_or(0);

        let opts = GlobOptions {
            pattern: pattern.to_string(),
            path: search_path,
            type_filter: input.type_filter.clone(),
            max_results: 10_000,
        };

        let result = ctx.backend.glob(&opts).await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(e.to_string()))
        })?;

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
