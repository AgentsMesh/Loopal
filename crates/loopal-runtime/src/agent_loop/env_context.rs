//! Dynamic per-turn environment context injection.
//!
//! Appends a lightweight `# Environment` section to the system prompt
//! on each LLM call, providing the model with current date, working
//! directory, git branch, and turn progress.

use std::path::Path;

/// Build a dynamic environment section (~100 tokens).
pub fn build_env_section(cwd: &Path, turn_count: u32, max_turns: u32) -> String {
    let mut parts = Vec::with_capacity(4);

    // Date/time
    let now = chrono::Local::now();
    parts.push(format!("- Date: {}", now.format("%Y-%m-%d %H:%M %Z")));

    // Working directory
    parts.push(format!("- Working directory: {}", cwd.display()));

    // Git branch (fail silently if not in a repo)
    if let Some(branch) = loopal_git::current_branch(cwd) {
        parts.push(format!("- Git branch: {branch}"));
    }

    // Turn progress
    parts.push(format!("- Turn: {turn_count}/{max_turns}"));

    format!("\n\n# Environment\n{}", parts.join("\n"))
}
