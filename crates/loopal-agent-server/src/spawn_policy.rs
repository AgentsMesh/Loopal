use std::collections::HashSet;

use loopal_kernel::Kernel;

const SPAWN_TOOLS: &[&str] = &["Agent", "SendMessage", "ListHubs"];

/// Build a tool whitelist that excludes spawn-related tools.
///
/// Returns `None` when the agent is still within the allowed depth
/// (spawn tools remain available). Returns `Some(filter)` when
/// `depth >= max_depth`, physically preventing further sub-agent creation.
pub fn build_depth_tool_filter(
    kernel: &Kernel,
    depth: u32,
    max_depth: u32,
) -> Option<HashSet<String>> {
    if depth < max_depth {
        return None;
    }
    let mut allowed: HashSet<String> = kernel
        .tool_definitions()
        .into_iter()
        .map(|t| t.name)
        .collect();
    for name in SPAWN_TOOLS {
        allowed.remove(*name);
    }
    Some(allowed)
}
