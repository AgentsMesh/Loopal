use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::path::PathBuf;

use super::grep_search::{OutputMode, format_results, search_files};

pub struct GrepTool;

/// Default head_limit caps the number of output entries returned.
const DEFAULT_HEAD_LIMIT: usize = 50;
/// Absolute maximum matches collected during search.
const MAX_TOTAL_MATCHES: usize = 500;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "Search file contents using a regex pattern. \
         Default output_mode is files_with_matches (file paths with match counts)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (default: cwd)"
                },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g. \"*.rs\")"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output format (default: files_with_matches)"
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Max output entries (default: 50)"
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

        if pattern.len() > 1000 {
            return Ok(ToolResult::error("pattern too long (max 1000 characters)"));
        }

        let re = RegexBuilder::new(pattern)
            .size_limit(1_000_000)
            .build()
            .map_err(|e| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                    format!("Invalid regex: {}", e),
                ))
            })?;

        let search_path = match input["path"].as_str() {
            Some(p) => {
                let pb = PathBuf::from(p);
                if pb.is_absolute() { pb } else { ctx.cwd.join(pb) }
            }
            None => ctx.cwd.clone(),
        };

        let include_glob = match input["include"].as_str() {
            Some(g) => {
                let glob = globset::Glob::new(g).map_err(|e| {
                    LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                        format!("Invalid include glob: {}", e),
                    ))
                })?;
                Some(glob.compile_matcher())
            }
            None => None,
        };

        let mode = OutputMode::from_str_opt(input["output_mode"].as_str())?;
        let head_limit = input["head_limit"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_HEAD_LIMIT);

        let results = search_files(
            &search_path, &re, include_glob.as_ref(), MAX_TOTAL_MATCHES,
        );
        let output = format_results(&results, mode, head_limit, MAX_TOTAL_MATCHES);
        Ok(ToolResult::success(output))
    }
}
