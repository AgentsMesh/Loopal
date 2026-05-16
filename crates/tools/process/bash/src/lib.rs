mod bg_convert;
mod bg_monitor;
mod env_inject;
pub mod format;
pub mod strategy;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{ExecOutcome, PermissionLevel, ToolContext, ToolResult, TypedTool};
use loopal_tool_background::BackgroundTaskStore;
use schemars::JsonSchema;
use serde::Deserialize;

use loopal_config::CommandDecision;
use loopal_sandbox::command_checker::check_command;
use loopal_sandbox::security_inspector::{SecurityVerdict, inspect_command};

use crate::env_inject::{build_env_override, validate_env};

pub struct BashTool {
    store: Arc<BackgroundTaskStore>,
}

impl BashTool {
    pub fn new(store: Arc<BackgroundTaskStore>) -> Self {
        Self { store }
    }
}

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const MAX_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Deserialize, JsonSchema)]
pub struct BashParams {
    pub command: String,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub run_in_background: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    /// Extra environment variables for the command. Values may contain
    /// `<secret_ref:NAME>` placeholders that the runtime substitutes before
    /// exec. Keys must match `^[A-Z_][A-Z0-9_]*$` and cannot override
    /// PATH/LD_*/DYLD_*/HOME.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

#[async_trait]
impl TypedTool<BashParams> for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Executes a given bash command and returns its output.\n\n\
         The working directory persists between commands, but shell state does not.\n\n\
         IMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, \
         or `echo` commands, unless explicitly instructed or after you have verified that a dedicated tool \
         cannot accomplish your task. Instead, use the appropriate dedicated tool:\n\
         - File search: Use Glob (NOT find or ls)\n\
         - Content search: Use Grep (NOT grep or rg)\n\
         - Read files: Use Read (NOT cat/head/tail)\n\
         - Edit files: Use Edit (NOT sed/awk)\n\
         - Write files: Use Write (NOT echo >/cat <<EOF)\n\n\
         # Instructions\n\
         - Always quote file paths that contain spaces with double quotes.\n\
         - Try to maintain your current working directory by using absolute paths and avoiding `cd`.\n\
         - You may specify an optional timeout in seconds (up to 600s / 10 minutes). Default timeout is 300s (5 minutes).\n\
         - Use `run_in_background` for long-running commands. You will be notified when they finish — do not poll.\n\
         - To check on or stop a background process, use the BashProcess tool.\n\
         - When issuing multiple commands:\n\
           - If independent and can run in parallel, make multiple Bash tool calls in a single message.\n\
           - If dependent, chain them with '&&' in a single Bash call.\n\
           - DO NOT use newlines to separate commands.\n\
         - Avoid unnecessary `sleep` commands — diagnose root causes instead of retry loops.\n\
         - For git commands: prefer new commits over amending; never skip hooks (--no-verify) unless asked.\n\
         - For secrets: use the `env` field with `<secret_ref:NAME>` placeholders\n\
           (e.g. `env: { TOKEN: \"<secret_ref:openai_key>\" }`) instead of inlining \
           secrets into `command` — env injection avoids leaking values to `ps`."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Dangerous
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &["command", "env"]
    }

    fn precheck(&self, input: &BashParams) -> Option<String> {
        if let Some(reason) = validate_env(input.env.as_ref()) {
            return Some(reason);
        }
        if let CommandDecision::Deny(reason) = check_command(&input.command) {
            return Some(reason);
        }
        if let SecurityVerdict::Block(reason) = inspect_command(&input.command) {
            return Some(reason);
        }
        None
    }

    async fn execute(
        &self,
        input: BashParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let env = build_env_override(input.env.as_ref());
        if input.run_in_background.unwrap_or(false) {
            let desc = input.description.as_deref().unwrap_or(&input.command);
            return match ctx.backend.exec_background(&input.command, &env).await {
                Ok(handle) => {
                    let task_id = bg_convert::register_spawned(&self.store, handle, desc)
                        .unwrap_or_else(|| "(unknown)".into());
                    Ok(ToolResult::success(format!(
                        "Background process started.\nprocess_id: {task_id}"
                    )))
                }
                Err(e) => Ok(ToolResult::error(e.to_string())),
            };
        }

        exec_foreground(&self.store, &input, &env, ctx).await
    }
}

async fn exec_foreground(
    store: &BackgroundTaskStore,
    input: &BashParams,
    env: &loopal_tool_api::backend_types::EnvOverride,
    ctx: &ToolContext,
) -> Result<ToolResult, LoopalError> {
    let timeout_secs = input.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS);
    let timeout = Duration::from_secs(timeout_secs).min(MAX_TIMEOUT);

    let exec_result = if let Some(ref tail) = ctx.output_tail {
        ctx.backend
            .exec_streaming(&input.command, timeout, env, tail.clone())
            .await
    } else {
        ctx.backend
            .exec(&input.command, timeout, env)
            .await
            .map(ExecOutcome::Completed)
    };

    match exec_result {
        Ok(ExecOutcome::Completed(output)) => {
            Ok(format::format_exec_result(output, &input.command))
        }
        Ok(ExecOutcome::TimedOut {
            timeout,
            partial_output,
            handle,
        }) => {
            let task_id = bg_convert::register(store, handle, &input.command)
                .unwrap_or_else(|| "(unknown)".into());
            Ok(format::format_converted_to_background(
                &task_id,
                timeout,
                &partial_output,
            ))
        }
        Err(loopal_error::ToolIoError::Timeout(d)) => {
            Err(LoopalError::Tool(loopal_error::ToolError::Timeout(d)))
        }
        Err(e) => Ok(ToolResult::error(e.to_string())),
    }
}
