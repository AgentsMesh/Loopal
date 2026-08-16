use std::sync::Arc;
use std::time::Duration;

use loopal_error::{ProcessHandle, ToolIoError};
use loopal_tool_api::{EnvOverride, ExecOutcome, ExecResult, OutputTail, ToolContext};

pub(crate) async fn foreground(
    ctx: &ToolContext,
    command: &str,
    timeout: Duration,
    env: &EnvOverride,
) -> Result<ExecResult, ToolIoError> {
    if let Some(executor) = &ctx.process_executor {
        executor.exec(command, timeout, env).await
    } else {
        ctx.backend.exec(command, timeout, env).await
    }
}

pub(crate) async fn streaming(
    ctx: &ToolContext,
    command: &str,
    timeout: Duration,
    env: &EnvOverride,
    tail: Arc<OutputTail>,
) -> Result<ExecOutcome, ToolIoError> {
    if let Some(executor) = &ctx.process_executor {
        executor.exec_streaming(command, timeout, env, tail).await
    } else {
        ctx.backend
            .exec_streaming(command, timeout, env, tail)
            .await
    }
}

pub(crate) async fn background(
    ctx: &ToolContext,
    command: &str,
    env: &EnvOverride,
) -> Result<ProcessHandle, ToolIoError> {
    if let Some(executor) = &ctx.process_executor {
        executor.exec_background(command, env).await
    } else {
        ctx.backend.exec_background(command, env).await
    }
}

#[cfg(test)]
#[path = "process_exec_tests.rs"]
mod tests;
