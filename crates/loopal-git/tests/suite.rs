// Single test binary — includes all test modules
#[path = "suite/repo_test.rs"]
mod repo_test;
#[path = "suite/worktree_test.rs"]
mod worktree_test;
#[path = "suite/validate_test.rs"]
mod validate_test;

/// Create a fresh git repo in a tempdir with one initial commit.
fn init_repo(dir: &std::path::Path) {
    run(dir, &["git", "init"]);
    run(dir, &["git", "config", "user.email", "test@test.com"]);
    run(dir, &["git", "config", "user.name", "Test"]);
    std::fs::write(dir.join("README.md"), "init").unwrap();
    run(dir, &["git", "add", "."]);
    run(dir, &["git", "commit", "-m", "init"]);
}

fn run(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new(args[0])
        .args(&args[1..])
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "{args:?} failed in {}", dir.display());
}
