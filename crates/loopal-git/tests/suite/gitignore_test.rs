use loopal_git::ensure_loopal_gitignore;
use std::fs;

const EXPECTED: &str = "\
# Auto-managed by Loopal. Do not edit.
worktrees/
plans/
settings.local.json
LOOPAL.local.md
";

#[test]
fn writes_when_inside_git_worktree() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    let loopal = dir.path().join(".loopal");
    fs::create_dir(&loopal).unwrap();

    ensure_loopal_gitignore(&loopal);

    let content = fs::read_to_string(loopal.join(".gitignore")).unwrap();
    assert_eq!(content, EXPECTED);
}

#[test]
fn noop_when_not_in_git_worktree() {
    let dir = tempfile::tempdir().unwrap();
    let loopal = dir.path().join(".loopal");
    fs::create_dir(&loopal).unwrap();

    ensure_loopal_gitignore(&loopal);

    assert!(!loopal.join(".gitignore").exists());
}

#[test]
fn idempotent_when_content_matches() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    let loopal = dir.path().join(".loopal");
    fs::create_dir(&loopal).unwrap();

    ensure_loopal_gitignore(&loopal);
    let before = fs::metadata(loopal.join(".gitignore"))
        .unwrap()
        .modified()
        .unwrap();

    // Force a measurable time gap so any rewrite would show.
    std::thread::sleep(std::time::Duration::from_millis(20));
    ensure_loopal_gitignore(&loopal);
    let after = fs::metadata(loopal.join(".gitignore"))
        .unwrap()
        .modified()
        .unwrap();

    assert_eq!(before, after, "matching content must not be rewritten");
}

#[test]
fn overwrites_stale_user_edited_content() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    let loopal = dir.path().join(".loopal");
    fs::create_dir(&loopal).unwrap();
    fs::write(loopal.join(".gitignore"), "stale-user-edit\n").unwrap();

    ensure_loopal_gitignore(&loopal);

    let content = fs::read_to_string(loopal.join(".gitignore")).unwrap();
    assert_eq!(content, EXPECTED);
}

#[test]
fn detects_git_worktree_dot_git_as_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".git"), "gitdir: /elsewhere\n").unwrap();
    let loopal = dir.path().join(".loopal");
    fs::create_dir(&loopal).unwrap();

    ensure_loopal_gitignore(&loopal);

    let content = fs::read_to_string(loopal.join(".gitignore")).unwrap();
    assert_eq!(content, EXPECTED);
}

#[test]
fn finds_git_via_parent_traversal() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    let nested = dir.path().join("a").join("b").join(".loopal");
    fs::create_dir_all(&nested).unwrap();

    ensure_loopal_gitignore(&nested);

    assert!(nested.join(".gitignore").exists());
}
