use std::fs;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use loopal_context::middleware::file_snapshot::{FileSnapshot, format_file_change, line_diff};

/// Sleep enough for filesystem mtime to advance (NTFS has ~100ms granularity,
/// but Windows CI can be slow — 1.1s covers HFS+ 1-second granularity too).
fn wait_for_mtime() {
    sleep(Duration::from_millis(1100));
}

#[test]
fn line_diff_empty_to_content() {
    let (added, removed) = line_diff("", "line1\nline2");
    assert_eq!(added, vec!["line1", "line2"]);
    assert!(removed.is_empty());
}

#[test]
fn line_diff_content_to_empty() {
    let (added, removed) = line_diff("line1\nline2", "");
    assert!(added.is_empty());
    assert_eq!(removed, vec!["line1", "line2"]);
}

#[test]
fn line_diff_no_change() {
    let (added, removed) = line_diff("same\nlines", "same\nlines");
    assert!(added.is_empty());
    assert!(removed.is_empty());
}

#[test]
fn line_diff_partial_change() {
    let (added, removed) = line_diff("keep\nold\nstay", "keep\nnew\nstay");
    assert_eq!(added, vec!["new"]);
    assert_eq!(removed, vec!["old"]);
}

#[test]
fn line_diff_blank_lines_ignored() {
    let (added, removed) = line_diff("a\n\nb", "a\n\n\nb");
    assert!(added.is_empty());
    assert!(removed.is_empty());
}

#[test]
fn line_diff_preserves_duplicates() {
    let (added, removed) = line_diff("a\na\nb", "a\nb");
    assert!(added.is_empty());
    assert_eq!(removed, vec!["a"]);

    let (added2, removed2) = line_diff("a\nb", "a\na\nb");
    assert_eq!(added2, vec!["a"]);
    assert!(removed2.is_empty());
}

#[test]
fn line_diff_unicode() {
    let (added, removed) = line_diff("你好\n世界", "你好\n新行");
    assert_eq!(added, vec!["新行"]);
    assert_eq!(removed, vec!["世界"]);
}

#[test]
fn format_added_only() {
    let result = format_file_change("Test", &["new line"], &[]);
    assert!(result.contains("[Config Update] Test changed:"));
    assert!(result.contains("+ new line"));
    assert!(!result.contains("Removed"));
}

#[test]
fn format_removed_only() {
    let result = format_file_change("Test", &[], &["old line"]);
    assert!(result.contains("- old line"));
    assert!(!result.contains("Added"));
}

#[test]
fn format_truncates_long_added() {
    let lines: Vec<&str> = (0..20).map(|_| "x").collect();
    let result = format_file_change("T", &lines, &[]);
    assert!(result.contains("and 5 more lines"));
}

#[test]
fn snapshot_nonexistent_file_no_change() {
    let mut snap = FileSnapshot::load(PathBuf::from("/tmp/loopal_test_noexist_xyz"), "Missing");
    assert!(snap.check_and_refresh().is_none());
}

#[test]
fn snapshot_detects_file_creation() {
    let dir = std::env::temp_dir().join("loopal_snap_create_v2");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");

    let mut snap = FileSnapshot::load(path.clone(), "Test");
    assert!(snap.check_and_refresh().is_none());

    fs::write(&path, "new content").unwrap();
    let reminder = snap.check_and_refresh();
    assert!(reminder.is_some());
    assert!(reminder.unwrap().contains("new content"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_detects_modification() {
    let dir = std::env::temp_dir().join("loopal_snap_modify_v2");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "original").unwrap();

    let mut snap = FileSnapshot::load(path.clone(), "Test");

    wait_for_mtime();
    fs::write(&path, "updated").unwrap();
    let reminder = snap.check_and_refresh();
    assert!(reminder.is_some());
    let text = reminder.unwrap();
    assert!(text.contains("+ updated"));
    assert!(text.contains("- original"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_detects_deletion() {
    let dir = std::env::temp_dir().join("loopal_snap_delete_v2");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "content").unwrap();

    let mut snap = FileSnapshot::load(path.clone(), "Test");
    fs::remove_file(&path).unwrap();

    let reminder = snap.check_and_refresh();
    assert!(reminder.is_some());
    assert!(reminder.unwrap().contains("- content"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_stable_returns_none() {
    let dir = std::env::temp_dir().join("loopal_snap_stable_v2");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "stable").unwrap();

    let mut snap = FileSnapshot::load(path, "Test");
    assert!(snap.check_and_refresh().is_none());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_sequential_changes_both_detected() {
    let dir = std::env::temp_dir().join("loopal_snap_seq_v2");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mem.md");
    fs::write(&path, "v1").unwrap();

    let mut snap = FileSnapshot::load(path.clone(), "Test");

    wait_for_mtime();
    fs::write(&path, "v2").unwrap();
    assert!(snap.check_and_refresh().is_some());

    wait_for_mtime();
    fs::write(&path, "v3").unwrap();
    let r = snap.check_and_refresh();
    assert!(r.is_some());
    assert!(r.unwrap().contains("v3"));

    let _ = fs::remove_dir_all(&dir);
}
