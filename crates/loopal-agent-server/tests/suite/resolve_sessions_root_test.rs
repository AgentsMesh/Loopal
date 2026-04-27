//! Tests for `resolve_sessions_root` — the path-resolution policy that
//! decides where session-scoped storage lives.

use loopal_agent_server::testing::resolve_sessions_root;
use tempfile::tempdir;

#[test]
fn override_path_takes_precedence() {
    let dir = tempdir().unwrap();
    let resolved = resolve_sessions_root(Some(dir.path()));
    assert_eq!(resolved, dir.path().to_path_buf());
}

#[test]
fn no_override_falls_back_to_global_or_temp() {
    let resolved = resolve_sessions_root(None);
    // Either the production global location (`~/.loopal/sessions`) or
    // the sandbox fallback (`<temp>/loopal/sessions`). We don't assert
    // on which one — Bazel test sandbox lacks HOME and falls back to
    // temp; CI may differ. We assert only that it's non-empty and ends
    // with "sessions".
    let s = resolved.to_string_lossy();
    assert!(!s.is_empty());
    assert!(
        s.ends_with("sessions"),
        "resolved root must end with 'sessions': {s}"
    );
}

#[test]
fn override_with_nested_path_preserved_verbatim() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a").join("b");
    std::fs::create_dir_all(&nested).unwrap();
    let resolved = resolve_sessions_root(Some(&nested));
    assert_eq!(resolved, nested);
}
