//! System prompt post-processing: appends MCP, scheduler, and resource sections.

use loopal_kernel::Kernel;

const SCHEDULER_PROMPT: &str = "\n\n# Scheduled Messages\n\
    Messages prefixed with `[scheduled]` are injected by the cron scheduler, \
    not typed by the user. Treat them as automated prompts and execute the \
    requested action without asking for confirmation. \
    Use CronCreate/CronDelete/CronList tools to manage scheduled jobs.";

/// Append MCP instructions, scheduler guidance, and resource/prompt summaries.
pub fn append_runtime_sections(prompt: &mut String, kernel: &Kernel) {
    let mcp_instructions = kernel.mcp_instructions();
    if !mcp_instructions.is_empty() {
        prompt.push_str("\n\n# MCP Server Instructions\n");
        for (server_name, instructions) in mcp_instructions {
            prompt.push_str(&format!("\n## {server_name}\n{instructions}\n"));
        }
    }

    prompt.push_str(SCHEDULER_PROMPT);

    let mcp_resources = kernel.mcp_resources();
    if !mcp_resources.is_empty() {
        prompt.push_str("\n\n# Available MCP Resources\n");
        for (server, res) in mcp_resources {
            let desc = res.description.as_deref().unwrap_or("");
            prompt.push_str(&format!("\n- `{}` ({server}): {desc}", res.uri));
        }
    }

    let mcp_prompts = kernel.mcp_prompts();
    if !mcp_prompts.is_empty() {
        prompt.push_str("\n\n# Available MCP Prompts\n");
        for (server, p) in mcp_prompts {
            let desc = p.description.as_deref().unwrap_or("");
            prompt.push_str(&format!("\n- `{}` ({server}): {desc}", p.name));
        }
    }
}
