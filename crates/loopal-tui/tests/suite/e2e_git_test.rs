//! E2E git fixture tests: repo creation, file commits, branch operations.

use loopal_test_support::GitFixture;

#[test]
fn test_git_fixture_creates_repo() {
    let fixture = GitFixture::new();
    let git_dir = fixture.path().join(".git");
    assert!(git_dir.exists(), ".git directory should exist");

    // Verify it's a valid git repo by checking HEAD
    let head = fixture.path().join(".git/HEAD");
    assert!(head.exists(), "HEAD should exist in .git");
}

#[test]
fn test_git_fixture_has_initial_commit() {
    let fixture = GitFixture::new();

    // README.md was created in the initial commit
    let readme = fixture.path().join("README.md");
    assert!(readme.exists(), "README.md should exist");
    assert_eq!(
        std::fs::read_to_string(&readme).unwrap(),
        "init",
        "README.md should have initial content"
    );
}

#[test]
fn test_git_fixture_commit_file() {
    let fixture = GitFixture::new();
    fixture.commit_file("src/main.rs", "fn main() {}", "add main");

    let file = fixture.path().join("src/main.rs");
    assert!(file.exists(), "committed file should exist on disk");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "fn main() {}",
        "file content should match"
    );

    // Verify it was actually committed (working tree clean)
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(fixture.path())
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.trim().is_empty(),
        "working tree should be clean after commit, got: {status}"
    );
}

#[test]
fn test_git_fixture_current_branch() {
    let fixture = GitFixture::new();
    let branch = fixture.current_branch();
    // Git default branch is typically "main" or "master"
    assert!(
        branch == "main" || branch == "master",
        "expected main or master, got: {branch}"
    );
}

#[test]
fn test_git_fixture_multiple_commits() {
    let fixture = GitFixture::new();
    fixture.commit_file("a.txt", "aaa", "add a");
    fixture.commit_file("b.txt", "bbb", "add b");
    fixture.commit_file("c.txt", "ccc", "add c");

    // Verify all 3 files exist
    assert!(fixture.path().join("a.txt").exists());
    assert!(fixture.path().join("b.txt").exists());
    assert!(fixture.path().join("c.txt").exists());

    // Verify commit count (initial + 3 = 4)
    let output = std::process::Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(fixture.path())
        .output()
        .unwrap();
    let count: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap();
    assert_eq!(count, 4, "expected 4 commits (1 initial + 3)");
}

#[test]
fn test_git_fixture_nested_dirs() {
    let fixture = GitFixture::new();
    fixture.commit_file("src/lib/mod.rs", "// module", "add nested");

    let file = fixture.path().join("src/lib/mod.rs");
    assert!(file.exists(), "nested file should exist");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "// module");
}
