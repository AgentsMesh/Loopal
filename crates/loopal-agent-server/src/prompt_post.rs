//! System prompt post-processing: appends MCP, scheduler, and resource sections.

use loopal_kernel::Kernel;

const SCHEDULER_PROMPT: &str = "\n\n# Scheduled Messages\n\
    Messages prefixed with `[scheduled]` are injected by the cron scheduler, \
    not typed by the user. Treat them as automated prompts and execute the \
    requested action without asking for confirmation. \
    Use CronCreate/CronDelete/CronList tools to manage scheduled jobs.";

/// Append MCP instructions, scheduler guidance, and resource/prompt summaries.
pub async fn append_runtime_sections(prompt: &mut String, kernel: &Kernel) {
    let mcp_instructions = kernel.mcp_instructions();
    if !mcp_instructions.is_empty() {
        prompt.push_str("\n\n# MCP Server Instructions\n");
        for (server_name, instructions) in mcp_instructions {
            prompt.push_str(&format!("\n## {server_name}\n{instructions}\n"));
        }
    }

    // reason: when a slow MCP server (chrome-devtools-mcp ~30s, etc.) misses
    // the bounded-wait budget, its tools won't be in the first-turn system
    // prompt and `tool_definitions()` will be incomplete. Without a hint
    // here, LLMs reflexively tell the user "I don't have that tool" instead
    // of suggesting they wait or check /mcp. Snapshot the configured server
    // names + statuses so the model knows what the user CONFIGURED even
    // when those tools haven't arrived yet.
    let configured: Vec<_> = kernel.settings().mcp_servers.keys().cloned().collect();
    if !configured.is_empty() {
        let snapshots = kernel.mcp_provider().snapshot().await;
        let mut by_name: std::collections::HashMap<String, String> =
            snapshots.into_iter().map(|s| (s.name, s.status)).collect();
        prompt.push_str("\n\n# MCP Server Status\n");
        prompt.push_str(
            "Configured at session start. A server may still be initializing \
             — if a tool the user asked for is not visible yet, the server is \
             likely loading. Suggest the user check `/mcp` page rather than \
             claiming the capability is missing.\n",
        );
        for name in configured {
            let status = by_name
                .remove(&name)
                .unwrap_or_else(|| "pending".to_string());
            prompt.push_str(&format!("- {name}: {status}\n"));
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
