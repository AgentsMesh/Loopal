//! Unit tests for PlanFile and plan_file helpers.

use loopal_runtime::plan_file::{PlanFile, build_plan_mode_filter, wrap_plan_reminder};

#[test]
fn new_creates_path_under_plans_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    let expected_segment: &std::path::Path = &std::path::PathBuf::from(".loopal").join("plans");
    let path = pf.path();
    assert!(
        path.to_string_lossy()
            .contains(expected_segment.to_string_lossy().as_ref()),
        "path {path:?} should contain {expected_segment:?}"
    );
    assert!(path.extension().is_some_and(|e| e == "md"));
}

#[test]
fn new_avoids_collision_with_existing_file() {
    let tmp = tempfile::tempdir().unwrap();
    let plans_dir = tmp.path().join(".loopal").join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();

    // Create a first plan file, then write it to disk to cause a collision.
    let first = PlanFile::new(tmp.path());
    std::fs::write(first.path(), "existing plan").unwrap();

    // Second plan file should pick a different path.
    let second = PlanFile::new(tmp.path());
    assert_ne!(first.path(), second.path());
}

#[test]
fn exists_returns_false_for_new_plan() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    assert!(!pf.exists());
}

#[test]
fn exists_returns_true_after_write() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    std::fs::create_dir_all(pf.path().parent().unwrap()).unwrap();
    std::fs::write(pf.path(), "my plan").unwrap();
    assert!(pf.exists());
}

#[test]
fn read_returns_none_when_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    assert!(pf.read().is_none());
}

#[test]
fn read_returns_none_for_empty_file() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    std::fs::create_dir_all(pf.path().parent().unwrap()).unwrap();
    std::fs::write(pf.path(), "").unwrap();
    assert!(pf.read().is_none());
}

#[test]
fn read_returns_none_for_whitespace_only() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    std::fs::create_dir_all(pf.path().parent().unwrap()).unwrap();
    std::fs::write(pf.path(), "   \n  \n  ").unwrap();
    assert!(pf.read().is_none());
}

#[test]
fn read_returns_content_when_present() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    std::fs::create_dir_all(pf.path().parent().unwrap()).unwrap();
    std::fs::write(pf.path(), "# My Plan\nStep 1\n").unwrap();
    let content = pf.read().unwrap();
    assert!(content.contains("My Plan"));
}

#[test]
fn matches_path_absolute_exact() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    let path_str = pf.path().to_string_lossy().to_string();
    assert!(pf.matches_path(&path_str));
}

#[test]
fn matches_path_relative() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    // Extract relative path from cwd
    let rel = pf
        .path()
        .strip_prefix(tmp.path())
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(pf.matches_path(&rel));
}

#[test]
fn matches_path_rejects_different_file() {
    let tmp = tempfile::tempdir().unwrap();
    let pf = PlanFile::new(tmp.path());
    assert!(!pf.matches_path("/some/other/file.md"));
    assert!(!pf.matches_path("random.txt"));
}

#[test]
fn wrap_plan_reminder_appends_system_reminder() {
    let result = wrap_plan_reminder("Tool output", "/plans/test.md");
    assert!(result.starts_with("Tool output"));
    assert!(result.contains("<system-reminder>"));
    assert!(result.contains("/plans/test.md"));
    assert!(result.contains("AskUser"));
    assert!(!result.contains("AskUserQuestion"));
}

#[test]
fn build_plan_mode_filter_includes_expected_tools() {
    let kernel = loopal_kernel::Kernel::new(loopal_config::Settings::default()).unwrap();
    let filter = build_plan_mode_filter(&kernel);

    // Special tools always included.
    assert!(filter.contains("Write"));
    assert!(filter.contains("Edit"));
    assert!(filter.contains("EnterPlanMode"));
    assert!(filter.contains("ExitPlanMode"));
    assert!(filter.contains("AskUser"));
    assert!(filter.contains("Agent"));

    // ReadOnly tools included.
    assert!(filter.contains("Read"));
    assert!(filter.contains("Glob"));
    assert!(filter.contains("Grep"));
    assert!(filter.contains("Ls"));

    // Dangerous tools NOT included.
    assert!(!filter.contains("Bash"));
}
