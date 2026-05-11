use std::collections::HashSet;

use loopal_kernel::Kernel;

const SPAWN_TOOLS: &[&str] = &["Agent", "SendMessage", "ListHubs"];

const SUB_AGENT_FORBIDDEN_TOOLS: &[&str] = &["AskUser"];

pub fn build_depth_tool_filter(
    kernel: &Kernel,
    depth: u32,
    max_depth: u32,
) -> Option<HashSet<String>> {
    let is_sub_agent = depth > 0;
    let exhausted_spawn_budget = depth >= max_depth;
    if !is_sub_agent && !exhausted_spawn_budget {
        return None;
    }
    let mut allowed: HashSet<String> = kernel
        .tool_definitions()
        .into_iter()
        .map(|t| t.name)
        .collect();
    if is_sub_agent {
        for name in SUB_AGENT_FORBIDDEN_TOOLS {
            allowed.remove(*name);
        }
    }
    if exhausted_spawn_budget {
        for name in SPAWN_TOOLS {
            allowed.remove(*name);
        }
    }
    Some(allowed)
}
