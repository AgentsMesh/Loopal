/// Reserved name for the root (top-level) agent. Sub-agents that omit
/// an explicit `parent` field default to this. Hub forwarding helpers
/// (e.g. MCP request routing) target this name.
pub const ROOT_AGENT_NAME: &str = "main";
