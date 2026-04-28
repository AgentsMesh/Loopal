//! Pure decision logic for the `Agent` tool spawn action — extracted so
//! the `target_hub` → `SpawnTarget` mapping and worktree-isolation gating
//! can be unit-tested without an `AgentShared` / `Kernel` fixture.

use std::path::PathBuf;

use crate::spawn::SpawnTarget;

/// True iff worktree isolation can be applied: only in-hub spawns own a
/// path on the caller's filesystem, so cross-hub spawns must skip it.
pub(super) fn worktree_allowed(target_hub: &Option<String>, isolation: Option<&str>) -> bool {
    target_hub.is_none() && isolation == Some("worktree")
}

/// Map `target_hub` (and the in-hub-only context: cwd / fork_context)
/// into a typed `SpawnTarget`. Cross-hub variants intentionally drop
/// `cwd_override` / `fork_context` because they cannot cross filesystem
/// trust boundaries; pass these only via the `InHub` arm.
pub(super) fn build_spawn_target(
    target_hub: Option<String>,
    cwd_override: Option<PathBuf>,
    fork_context: Option<Vec<loopal_message::Message>>,
) -> SpawnTarget {
    match target_hub {
        Some(hub_id) => SpawnTarget::CrossHub { hub_id },
        None => SpawnTarget::InHub {
            cwd_override,
            fork_context,
        },
    }
}

#[cfg(test)]
#[path = "spawn_decision_test.rs"]
mod tests;
