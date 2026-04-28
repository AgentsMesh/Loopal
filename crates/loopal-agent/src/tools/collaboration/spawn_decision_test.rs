//! Unit tests for `spawn_decision::worktree_allowed` / `build_spawn_target`.

use super::{build_spawn_target, worktree_allowed};
use crate::spawn::SpawnTarget;
use std::path::PathBuf;

#[test]
fn worktree_allowed_only_for_inhub() {
    assert!(worktree_allowed(&None, Some("worktree")));
    assert!(!worktree_allowed(&Some("hub-b".into()), Some("worktree")));
    assert!(!worktree_allowed(&None, None));
    assert!(!worktree_allowed(&None, Some("other")));
}

#[test]
fn target_hub_some_yields_crosshub_dropping_cwd_and_fork_context() {
    let target = build_spawn_target(
        Some("hub-b".into()),
        Some(PathBuf::from("/local/wt")),
        Some(vec![loopal_message::Message::user("would-be ctx")]),
    );
    match target {
        SpawnTarget::CrossHub { hub_id } => assert_eq!(hub_id, "hub-b"),
        _ => panic!("expected CrossHub"),
    }
}

#[test]
fn target_hub_none_yields_inhub_carrying_cwd_and_fork_context() {
    let target = build_spawn_target(
        None,
        Some(PathBuf::from("/local/wt")),
        Some(vec![loopal_message::Message::user("ctx")]),
    );
    match target {
        SpawnTarget::InHub {
            cwd_override,
            fork_context,
        } => {
            assert_eq!(cwd_override, Some(PathBuf::from("/local/wt")));
            assert_eq!(fork_context.map(|v| v.len()), Some(1));
        }
        _ => panic!("expected InHub"),
    }
}

#[test]
fn inhub_with_no_overrides_passes_none_through() {
    let target = build_spawn_target(None, None, None);
    match target {
        SpawnTarget::InHub {
            cwd_override,
            fork_context,
        } => {
            assert!(cwd_override.is_none());
            assert!(fork_context.is_none());
        }
        _ => panic!("expected InHub"),
    }
}
