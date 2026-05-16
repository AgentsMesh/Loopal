use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{LsEntry, PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::ls_format;

pub struct LsTool;

#[derive(Deserialize, JsonSchema)]
pub struct LsParams {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub long: Option<bool>,
    #[serde(default)]
    pub all: Option<bool>,
}

#[async_trait]
impl TypedTool<LsParams> for LsTool {
    fn name(&self) -> &str {
        "Ls"
    }

    fn description(&self) -> &str {
        "List directory contents with file names, sizes, and types.\n\
         - Use long mode for detailed info: size, permissions, and modification time.\n\
         - When path points to a file, shows detailed file info (stat).\n\
         - Use this tool instead of running `ls` via Bash — it provides structured output."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(&self, input: LsParams, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let raw_path = input.path.as_deref().unwrap_or(".");

        let target = match ctx.backend.resolve_path(raw_path, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let info = match ctx.backend.file_info(target.to_str().unwrap_or(".")).await {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        if !info.is_dir {
            return Ok(ToolResult::success(ls_format::format_stat_from_info(
                &target, &info,
            )));
        }

        let long = input.long.unwrap_or(false);
        let show_all = input.all.unwrap_or(false);

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
