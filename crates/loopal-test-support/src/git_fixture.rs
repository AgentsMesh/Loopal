//! Git repository fixture for worktree integration tests.

use std::path::Path;
use std::process::Command;

/// RAII git repo in a temporary directory. Drop auto-cleans.
pub struct GitFixture {
    dir: tempfile::TempDir,
}

impl GitFixture {
    /// Create a new git repo with an initial commit.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create git fixture tempdir");
        let path = dir.path();

        run_git(path, &["init"]);
        run_git(path, &["config", "user.name", "Test"]);
        run_git(path, &["config", "user.email", "test@test.com"]);

        std::fs::write(path.join("README.md"), "init").expect("write initial file");
        run_git(path, &["add", "."]);
        run_git(path, &["commit", "-m", "initial commit"]);

        Self { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Create a file, add, and commit it.
    pub fn commit_file(&self, relative_path: &str, content: &str, message: &str) {
        let file_path = self.dir.path().join(relative_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&file_path, content).expect("write fixture file");
        run_git(self.dir.path(), &["add", relative_path]);
        run_git(self.dir.path(), &["commit", "-m", message]);
    }

    /// Get current branch name.
    pub fn current_branch(&self) -> String {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(self.dir.path())
            .output()
            .expect("git rev-parse");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

impl Default for GitFixture {
    fn default() -> Self {
        Self::new()
    }
}

fn run_git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("git {}: {e}", args.join(" ")));
    assert!(
        status.success(),
        "git {} failed in {}",
        args.join(" "),
        cwd.display()
    );
}
