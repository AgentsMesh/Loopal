use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "Search file contents using a regex pattern. Returns matching lines with file paths and line numbers."
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

        let mut results = Vec::new();
        let max_results = 500;

        let entries: Vec<_> = if search_path.is_file() {
            vec![search_path.clone()]
        } else {
            WalkDir::new(&search_path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.into_path())
                .collect()
        };

        'outer: for path in entries {
            if let Some(ref glob_matcher) = include_glob
                && let Some(name) = path.file_name()
                    && !glob_matcher.is_match(name) {
                        continue;
                    }

            // Skip binary files
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    results.push(format!("{}:{}: {}", path.display(), line_num + 1, line));
                    if results.len() >= max_results {
                        break 'outer;
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(ToolResult::success("No matches found."))
        } else {
            let mut output = results.join("\n");
            if results.len() >= max_results {
                output.push_str(&format!("\n... (truncated at {} matches)", max_results));
            }
            Ok(ToolResult::success(output))
        }
    }
}
