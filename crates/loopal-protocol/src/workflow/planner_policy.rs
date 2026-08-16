/// Goals below this size are eligible for the deterministic direct policy.
pub const SIMPLE_GOAL_MAX_BYTES: usize = 240;
pub const SIMPLE_GOAL_MAX_LINES: usize = 4;

/// Conservative, provider-independent direct policy. It only classifies
/// plainly small goals; all ambiguous or larger goals remain planner-driven.
pub fn is_deterministically_simple_goal(goal: &str) -> bool {
    let trimmed = goal.trim();
    if trimmed.is_empty()
        || !trimmed.is_ascii()
        || trimmed.len() > SIMPLE_GOAL_MAX_BYTES
        || trimmed.lines().count() > SIMPLE_GOAL_MAX_LINES
    {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    [
        "parallel",
        "multiple agent",
        "subagent",
        "sub-agent",
        "cross-check",
        "independently",
        "several tasks",
        "fan out",
    ]
    .iter()
    .all(|marker| !lower.contains(marker))
}
