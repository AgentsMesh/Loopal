use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use regex::RegexBuilder;
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::grep_format::{FormatOptions, format_results};
use crate::grep_search::{OutputMode, SearchOptions, search_files, type_to_extensions};

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
         Supports context lines (-A/-B/-C), case-insensitive (-i), multiline, \
         file type filter, and result pagination (offset). \
         Default output_mode is files_with_matches."
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
                "glob": {
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
                },
                "-A": {
                    "type": "integer",
                    "description": "Lines to show after each match (content mode)"
                },
                "-B": {
                    "type": "integer",
                    "description": "Lines to show before each match (content mode)"
                },
                "-C": {
                    "type": "integer",
                    "description": "Lines to show before and after each match (alias for -A and -B)"
                },
                "-i": {
                    "type": "boolean",
                    "description": "Case-insensitive search (default: false)"
                },
                "-n": {
                    "type": "boolean",
                    "description": "Show line numbers in content mode (default: true)"
                },
                "multiline": {
                    "type": "boolean",
                    "description": "Enable multiline matching (default: false)"
                },
                "type": {
                    "type": "string",
                    "description": "File type filter (e.g. js, py, rust, go)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Skip first N entries before applying head_limit"
                },
                "fixed_strings": {
                    "type": "boolean",
                    "description": "Treat pattern as a literal string, not a regex (default: false)"
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
        if pattern.len() > 1000 {
            return Ok(ToolResult::error("pattern too long (max 1000 characters)"));
        }

        let (re, search_opts, mode, head_limit, fmt_opts) = parse_params(&input, pattern)?;
        let search_path = resolve_path(&input, ctx)?;
        let include_glob = parse_include_glob(&input)?;

        let results = search_files(
            &search_path,
            &re,
            include_glob.as_ref(),
            MAX_TOTAL_MATCHES,
            &search_opts,
        );
        let output = format_results(&results, mode, head_limit, MAX_TOTAL_MATCHES, &fmt_opts);
        Ok(ToolResult::success(output))
    }
}

fn parse_params(
    input: &Value,
    pattern: &str,
) -> Result<
    (
        regex::Regex,
        SearchOptions,
        OutputMode,
        usize,
        FormatOptions,
    ),
    LoopalError,
> {
    let case_insensitive = input["-i"].as_bool().unwrap_or(false);
    let multiline = input["multiline"].as_bool().unwrap_or(false);
    let fixed_strings = input["fixed_strings"].as_bool().unwrap_or(false);

    let effective_pattern = if fixed_strings {
        regex::escape(pattern)
    } else {
        pattern.to_string()
    };
    let re = RegexBuilder::new(&effective_pattern)
        .case_insensitive(case_insensitive)
        .multi_line(multiline)
        .dot_matches_new_line(multiline)
        .size_limit(1_000_000)
        .build()
        .map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(format!(
                "Invalid regex: {e}"
            )))
        })?;

    let ctx_c = input["-C"].as_u64().map(|n| n as usize);
    let ctx_after = input["-A"]
        .as_u64()
        .map(|n| n as usize)
        .or(ctx_c)
        .unwrap_or(0);
    let ctx_before = input["-B"]
        .as_u64()
        .map(|n| n as usize)
        .or(ctx_c)
        .unwrap_or(0);

    let type_exts = input["type"].as_str().map(|t| {
        type_to_extensions(t)
            .map(|exts| exts.into_iter().map(String::from).collect())
            .unwrap_or_default() // unknown type → empty list → no matches
    });

    let search_opts = SearchOptions {
        context_before: ctx_before,
        context_after: ctx_after,
        multiline,
        type_extensions: type_exts,
    };

    let mode = OutputMode::from_str_opt(input["output_mode"].as_str())?;
    let head_limit = input["head_limit"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_HEAD_LIMIT);
    let fmt_opts = FormatOptions {
        show_line_numbers: input["-n"].as_bool().unwrap_or(true),
        offset: input["offset"].as_u64().map(|n| n as usize).unwrap_or(0),
        has_context: ctx_before > 0 || ctx_after > 0,
    };

    Ok((re, search_opts, mode, head_limit, fmt_opts))
}

fn resolve_path(input: &Value, ctx: &ToolContext) -> Result<PathBuf, LoopalError> {
    match input["path"].as_str() {
        Some(p) => ctx.backend.resolve_path(p, false).map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(e.to_string()))
        }),
        None => Ok(ctx.backend.cwd().to_path_buf()),
    }
}

fn parse_include_glob(input: &Value) -> Result<Option<globset::GlobMatcher>, LoopalError> {
    match input["glob"].as_str() {
        Some(g) => {
            let glob = globset::Glob::new(g).map_err(|e| {
                LoopalError::Tool(loopal_error::ToolError::InvalidInput(format!(
                    "Invalid include glob: {e}"
                )))
            })?;
            Ok(Some(glob.compile_matcher()))
        }
        None => Ok(None),
    }
}
