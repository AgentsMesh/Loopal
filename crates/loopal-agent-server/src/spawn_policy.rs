use std::collections::HashSet;

/// Tools that spawn or coordinate sub-agents. Removed from depth-exhausted
/// agents (they cannot spawn deeper) but kept for everyone else.
const SPAWN_TOOLS: &[&str] = &["Agent", "SendMessage", "ListHubs"];

/// Tools sub-agents must never see (they have no user to ask).
const SUB_AGENT_FORBIDDEN_TOOLS: &[&str] = &["AskUser"];

/// Returns the set of tool names a sub-agent / depth-exhausted agent must
/// NEVER see. reason: this used to snapshot `kernel.tool_definitions()`
/// into an allow-list, but that made late-registered MCP tools invisible
/// to sub-agents forever — a 30s chrome-devtools-mcp boot would mean the
/// sub-agent never gets chrome tools even though root does. Deny-list
/// semantics let llm_params retain the latest ToolRegistry contents and
/// only strip the explicitly-forbidden names.
///
/// Returns `None` for root agents that haven't hit the depth limit (no
/// filter needed at all).
pub fn build_depth_tool_filter(depth: u32, max_depth: u32) -> Option<HashSet<String>> {
    let is_sub_agent = depth > 0;
    let exhausted_spawn_budget = depth >= max_depth;
    if !is_sub_agent && !exhausted_spawn_budget {
        return None;
    }
    let mut forbidden: HashSet<String> = HashSet::new();
    if is_sub_agent {
        for name in SUB_AGENT_FORBIDDEN_TOOLS {
            forbidden.insert((*name).to_string());
        }
    }
    if exhausted_spawn_budget {
        for name in SPAWN_TOOLS {
            forbidden.insert((*name).to_string());
        }
    }
    Some(forbidden)
}
