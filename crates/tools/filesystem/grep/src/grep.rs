use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{GrepOptions, PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::grep_format::{FormatOptions, OutputMode, format_results};

pub struct GrepTool;

#[derive(Deserialize, JsonSchema)]
pub struct GrepParams {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub output_mode: Option<String>,
    #[serde(default)]
    pub head_limit: Option<u64>,
    #[serde(default, rename = "-A")]
    pub context_after: Option<u64>,
    #[serde(default, rename = "-B")]
    pub context_before: Option<u64>,
    #[serde(default, rename = "-C")]
    pub context_both: Option<u64>,
    #[serde(default, rename = "-i")]
    pub case_insensitive: Option<bool>,
    #[serde(default, rename = "-n")]
    pub show_line_numbers: Option<bool>,
    #[serde(default)]
    pub multiline: Option<bool>,
    #[serde(default, rename = "type")]
    pub type_filter: Option<String>,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub fixed_strings: Option<bool>,
}

const DEFAULT_HEAD_LIMIT: usize = 50;
const MAX_TOTAL_MATCHES: usize = 500;

#[async_trait]
impl TypedTool<GrepParams> for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "A powerful search tool built on ripgrep.\n\n\
         Usage:\n\
         - ALWAYS use Grep for search tasks. NEVER invoke `grep` or `rg` as a Bash command. \
         The Grep tool has been optimized for correct permissions and access.\n\
         - Supports full regex syntax (e.g., \"log.*Error\", \"function\\\\s+\\\\w+\").\n\
         - Filter files with glob parameter (e.g., \"*.js\", \"**/*.tsx\") or type parameter (e.g., \"js\", \"py\", \"rust\").\n\
         - Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \
         \"count\" shows match counts.\n\
         - Use Agent tool for open-ended searches requiring multiple rounds.\n\
         - Pattern syntax: Uses ripgrep (not grep) — literal braces need escaping \
         (use `interface\\\\{\\\\}` to find `interface{}` in Go code).\n\
         - Multiline matching: By default patterns match within single lines only. \
         For cross-line patterns like `struct \\\\{[\\\\s\\\\S]*?field`, use `multiline: true`."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        input: GrepParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let pattern = &input.pattern;
        if pattern.len() > 1000 {
            return Ok(ToolResult::error("pattern too long (max 1000 characters)"));
        }

        let (grep_opts, mode, head_limit, fmt_opts) = parse_params(&input, ctx)?;
        let results = ctx.backend.grep(&grep_opts).await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(e.to_string()))
        })?;
        let output = format_results(&results, mode, head_limit, MAX_TOTAL_MATCHES, &fmt_opts);
        Ok(ToolResult::success(output))
    }
}

fn parse_params(
    input: &GrepParams,
    ctx: &ToolContext,
) -> Result<(GrepOptions, OutputMode, usize, FormatOptions), LoopalError> {
    let ctx_c = input.context_both.map(|n| n as usize);
    let ctx_after = input
        .context_after
        .map(|n| n as usize)
        .or(ctx_c)
        .unwrap_or(0);
    let ctx_before = input
        .context_before
        .map(|n| n as usize)
        .or(ctx_c)
        .unwrap_or(0);

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

    let grep_opts = GrepOptions {
        pattern: input.pattern.clone(),
        path: search_path,
        glob_filter: input.glob.clone(),
        case_insensitive: input.case_insensitive.unwrap_or(false),
        multiline: input.multiline.unwrap_or(false),
        fixed_strings: input.fixed_strings.unwrap_or(false),
        context_before: ctx_before,
        context_after: ctx_after,
        type_filter: input.type_filter.clone(),
        max_matches: MAX_TOTAL_MATCHES,
    };

    let mode = OutputMode::from_str_opt(input.output_mode.as_deref())?;
    let head_limit = input
        .head_limit
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_HEAD_LIMIT);
    let fmt_opts = FormatOptions {
        show_line_numbers: input.show_line_numbers.unwrap_or(true),
        offset: input.offset.map(|n| n as usize).unwrap_or(0),
        has_context: ctx_before > 0 || ctx_after > 0,
    };

    Ok((grep_opts, mode, head_limit, fmt_opts))
}
