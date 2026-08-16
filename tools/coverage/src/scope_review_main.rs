#[path = "scope_review.rs"]
mod scope_review;

use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const HELP: &str = r#"Review changed Stage 0/workflow Rust files against coverage scope.

Usage:
  bazel run //tools/coverage:scope_review

Every changed production Rust file in a security/workflow boundary must either appear in
included_sources.txt or have a content-hashed rationale in scope_exclusions.txt. The
review unions the working tree with changes since LOOPAL_COVERAGE_BASE_REF (default:
origin/main), so committing cannot hide an omission. Editing an excluded file changes
its hash and fails review. Tests, fixtures, and unrelated crates are outside the boundary.
"#;

fn main() {
    if let Err(errors) = run() {
        eprintln!("coverage scope review failed:\n{}", errors.join("\n"));
        std::process::exit(1);
    }
}

fn run() -> Result<(), Vec<String>> {
    if env::args()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        print!("{HELP}");
        return Ok(());
    }
    let workspace = env::var("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .or_else(|_| env::current_dir())
        .map_err(|error| vec![format!("cannot determine workspace: {error}")])?;
    let included = load_sources(&workspace.join("tools/coverage/included_sources.txt"))?;
    let changed = changed_sources(&workspace)?;
    scope_review::review(
        changed,
        &included,
        &workspace.join("tools/coverage/scope_exclusions.txt"),
        &workspace,
    )?;
    println!("coverage scope review passed");
    Ok(())
}

fn changed_sources(workspace: &Path) -> Result<Vec<String>, Vec<String>> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .current_dir(workspace)
        .output()
        .map_err(|error| vec![format!("cannot run git status: {error}")])?;
    if !output.status.success() {
        return Err(vec![String::from_utf8_lossy(&output.stderr).into_owned()]);
    }
    let mut paths = Vec::new();
    for entry in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let text = String::from_utf8_lossy(entry);
        let raw = text.get(3..).unwrap_or_default();
        let path = raw.rsplit(" -> ").next().unwrap_or(raw).replace('\\', "/");
        paths.push(path);
    }
    let base = env::var("LOOPAL_COVERAGE_BASE_REF").unwrap_or_else(|_| "origin/main".into());
    let diff = Command::new("git")
        .args(["diff", "--name-only", "-z", &format!("{base}...HEAD")])
        .current_dir(workspace)
        .output()
        .map_err(|error| vec![format!("cannot diff coverage base {base}: {error}")])?;
    if !diff.status.success() {
        return Err(vec![format!(
            "cannot diff coverage base {base}: {}",
            String::from_utf8_lossy(&diff.stderr)
        )]);
    }
    paths.extend(
        diff.stdout
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| String::from_utf8_lossy(entry).replace('\\', "/")),
    );
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn load_sources(path: &Path) -> Result<BTreeSet<String>, Vec<String>> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| vec![format!("cannot read {}: {error}", path.display())])?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}
