//! Tests for sandbox_precheck: extract_paths, patch timestamp stripping.

use loopal_runtime::agent_loop::sandbox_precheck::extract_paths;
use serde_json::json;

// ── extract_paths for each tool ──────────────────────────────────

#[test]
fn write_extracts_file_path() {
    let input = json!({"file_path": "/etc/nginx.conf", "content": "data"});
    let paths = extract_paths("Write", &input);
    assert_eq!(paths, vec![("/etc/nginx.conf".to_string(), true)]);
}

#[test]
fn read_extracts_file_path_as_non_write() {
    let input = json!({"file_path": "/etc/hosts"});
    let paths = extract_paths("Read", &input);
    assert_eq!(paths, vec![("/etc/hosts".to_string(), false)]);
}

#[test]
fn edit_extracts_file_path() {
    let input = json!({"file_path": "src/main.rs", "old_string": "a", "new_string": "b"});
    let paths = extract_paths("Edit", &input);
    assert_eq!(paths, vec![("src/main.rs".to_string(), true)]);
}

#[test]
fn multi_edit_extracts_file_path() {
    let input = json!({"file_path": "src/lib.rs", "edits": []});
    let paths = extract_paths("MultiEdit", &input);
    assert_eq!(paths, vec![("src/lib.rs".to_string(), true)]);
}

#[test]
fn delete_extracts_path_field() {
    let input = json!({"path": "/tmp/old.txt"});
    let paths = extract_paths("Delete", &input);
    assert_eq!(paths, vec![("/tmp/old.txt".to_string(), true)]);
}

#[test]
fn move_file_extracts_src_and_dst() {
    let input = json!({"src": "/a/b", "dst": "/c/d"});
    let paths = extract_paths("MoveFile", &input);
    assert_eq!(paths.len(), 2);
    assert_eq!(paths[0], ("/a/b".to_string(), true));
    assert_eq!(paths[1], ("/c/d".to_string(), true));
}

#[test]
fn copy_file_src_is_read_dst_is_write() {
    let input = json!({"src": "/a/b", "dst": "/c/d"});
    let paths = extract_paths("CopyFile", &input);
    assert_eq!(paths[0], ("/a/b".to_string(), false)); // src: read
    assert_eq!(paths[1], ("/c/d".to_string(), true)); // dst: write
}

#[test]
fn unknown_tool_returns_empty() {
    let input = json!({"command": "ls -la"});
    assert!(extract_paths("Bash", &input).is_empty());
    assert!(extract_paths("Glob", &input).is_empty());
    assert!(extract_paths("UnknownMcpTool", &input).is_empty());
}

#[test]
fn missing_file_path_returns_empty() {
    let input = json!({"content": "data"});
    assert!(extract_paths("Write", &input).is_empty());
}

#[test]
fn null_file_path_returns_empty() {
    let input = json!({"file_path": null});
    assert!(extract_paths("Write", &input).is_empty());
}

#[test]
fn numeric_file_path_returns_empty() {
    let input = json!({"file_path": 123});
    assert!(extract_paths("Write", &input).is_empty());
}

// ── ApplyPatch path extraction ───────────────────────────────────

#[test]
fn apply_patch_extracts_star_lines() {
    let input = json!({
        "patch": "*** /etc/nginx.conf\n--- old\n+++ new\n@@ -1 +1 @@\n-old\n+new"
    });
    let paths = extract_paths("ApplyPatch", &input);
    assert_eq!(paths, vec![("/etc/nginx.conf".to_string(), true)]);
}

#[test]
fn apply_patch_strips_timestamp() {
    let input = json!({
        "patch": "*** /etc/nginx.conf\t2024-01-01 12:00:00\n"
    });
    let paths = extract_paths("ApplyPatch", &input);
    assert_eq!(paths, vec![("/etc/nginx.conf".to_string(), true)]);
}

#[test]
fn apply_patch_empty_patch_returns_empty() {
    let input = json!({"patch": ""});
    assert!(extract_paths("ApplyPatch", &input).is_empty());
}

#[test]
fn apply_patch_no_patch_field_returns_empty() {
    let input = json!({"file_path": "test.txt"});
    assert!(extract_paths("ApplyPatch", &input).is_empty());
}
