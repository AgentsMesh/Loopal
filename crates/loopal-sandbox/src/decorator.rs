//! SandboxedTool — Tool decorator that enforces sandbox policy.
//!
//! Wraps any `Tool` implementation with precheck validation and,
//! for Bash tools, OS-level sandboxed execution.

use async_trait::async_trait;
use loopal_config::{CommandDecision, PathDecision, ResolvedPolicy, SandboxPolicy};
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::Value;

use crate::bash_executor::execute_sandboxed_bash;
use crate::command_checker::check_command;
use crate::network::{self, check_domain};
use crate::path_checker::check_path;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Decorator that adds sandbox enforcement to any tool.
pub struct SandboxedTool {
    inner: Box<dyn Tool>,
    policy: ResolvedPolicy,
}

impl SandboxedTool {
    pub fn new(inner: Box<dyn Tool>, policy: ResolvedPolicy) -> Self {
        Self { inner, policy }
    }
}

#[async_trait]
impl Tool for SandboxedTool {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn permission(&self) -> PermissionLevel {
        self.inner.permission()
    }

    fn precheck(&self, input: &Value) -> Option<String> {
        // Delegate inner precheck first
        if let Some(reason) = self.inner.precheck(input) {
            return Some(reason);
        }
        // Sandbox-specific checks by tool type
        match self.inner.name() {
            "Bash" => precheck_bash(&self.policy, input),
            "Write" | "Edit" => precheck_write(&self.policy, input),
            "Read" | "Glob" | "Grep" | "Ls" => precheck_read(&self.policy, input),
            "Fetch" => precheck_web(&self.policy, input),
            _ => precheck_generic(&self.policy, input),
        }
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        // Bash: intercept and run through OS sandbox
        if self.inner.name() == "Bash" {
            let command = input["command"].as_str().unwrap_or_default();
            let timeout_ms = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);
            return execute_sandboxed_bash(&self.policy, command, &ctx.cwd, timeout_ms).await;
        }
        // All other tools: delegate directly
        self.inner.execute(input, ctx).await
    }
}

fn precheck_bash(policy: &ResolvedPolicy, input: &Value) -> Option<String> {
    // ReadOnly sandbox: block all Bash — shell commands can't be statically
    // guaranteed to be read-only, so we reject at precheck rather than relying
    // solely on OS sandbox at execution time.
    if policy.policy == SandboxPolicy::ReadOnly {
        return Some("read-only sandbox: Bash commands are blocked".into());
    }
    let cmd = input["command"].as_str().unwrap_or_default();
    if let CommandDecision::Deny(reason) = check_command(cmd) {
        return Some(reason);
    }
    None
}

fn precheck_write(policy: &ResolvedPolicy, input: &Value) -> Option<String> {
    let path_str = input["file_path"].as_str().unwrap_or_default();
    let path = std::path::Path::new(path_str);
    if let PathDecision::DenyWrite(reason) = check_path(policy, path, true) {
        return Some(reason);
    }
    None
}

fn precheck_read(policy: &ResolvedPolicy, input: &Value) -> Option<String> {
    let path_str = input["file_path"]
        .as_str()
        .or_else(|| input["path"].as_str());
    if let Some(p) = path_str {
        let path = std::path::Path::new(p);
        if let PathDecision::DenyRead(reason) =
            check_path(policy, path, false)
        {
            return Some(reason);
        }
    }
    None
}

fn precheck_web(policy: &ResolvedPolicy, input: &Value) -> Option<String> {
    if let Some(url) = input["url"].as_str()
        && let Some(domain) = network::extract_domain(url)
        && let Err(reason) = check_domain(&policy.network, &domain)
    {
        return Some(reason);
    }
    None
}

/// Generic fallback for MCP tools and unrecognized tools.
fn precheck_generic(policy: &ResolvedPolicy, input: &Value) -> Option<String> {
    for key in ["file_path", "path", "destination", "output_path", "target"] {
        if let Some(path_str) = input[key].as_str() {
            let path = std::path::Path::new(path_str);
            if let PathDecision::DenyWrite(reason) =
                check_path(policy, path, true)
            {
                return Some(reason);
            }
        }
    }
    for key in ["command", "cmd", "shell", "script", "exec"] {
        if let Some(cmd) = input[key].as_str()
            && let CommandDecision::Deny(reason) = check_command(cmd)
        {
            return Some(reason);
        }
    }
    for key in ["url", "endpoint", "uri", "href"] {
        if let Some(url) = input[key].as_str()
            && let Some(domain) = network::extract_domain(url)
            && let Err(reason) = check_domain(&policy.network, &domain)
        {
            return Some(reason);
        }
    }
    None
}
