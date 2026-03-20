use loopal_tool_api::{Tool, ToolContext};
use loopal_tools::builtin::grep::GrepTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    ToolContext { cwd: cwd.to_path_buf(), session_id: "test".into(), shared: None }
}

fn make_file(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

const FIVE_LINES: &str = "alpha\nbeta\ngamma\ndelta\nepsilon";

#[tokio::test]
async fn context_after_shows_lines_after_match() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "gamma", "output_mode": "content", "-A": 1}), &ctx)
        .await.unwrap();
    assert!(r.content.contains(":3:gamma"), "match line");
    assert!(r.content.contains("-4-delta"), "context after");
    assert!(!r.content.contains("epsilon"));
}

#[tokio::test]
async fn context_before_shows_lines_before_match() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "gamma", "output_mode": "content", "-B": 1}), &ctx)
        .await.unwrap();
    assert!(r.content.contains("-2-beta"), "context before");
    assert!(r.content.contains(":3:gamma"), "match line");
    assert!(!r.content.contains("alpha"));
}

#[tokio::test]
async fn context_c_sets_both_directions() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "gamma", "output_mode": "content", "-C": 1}), &ctx)
        .await.unwrap();
    assert!(r.content.contains("-2-beta"));
    assert!(r.content.contains(":3:gamma"));
    assert!(r.content.contains("-4-delta"));
}

#[tokio::test]
async fn context_merges_overlapping_ranges() {
    let tmp = tempfile::tempdir().unwrap();
    // Matches at line 2 (beta) and line 4 (delta); -C=1 → ranges [1,3] and [3,5] merge → [1,5]
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "beta|delta", "output_mode": "content", "-C": 1}),
            &ctx,
        )
        .await.unwrap();
    // All 5 lines should be in one contiguous group (no -- separator)
    assert!(!r.content.contains("--"), "ranges should merge, no separator");
    assert!(r.content.contains("alpha"));
    assert!(r.content.contains("epsilon"));
}

#[tokio::test]
async fn context_at_file_boundary() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());

    // Match first line with -B=5 → should not panic, just clamp
    let r = tool
        .execute(json!({"pattern": "alpha", "output_mode": "content", "-B": 5}), &ctx)
        .await.unwrap();
    assert!(r.content.contains(":1:alpha"));

    // Match last line with -A=5 → should not panic, just clamp
    let r = tool
        .execute(json!({"pattern": "epsilon", "output_mode": "content", "-A": 5}), &ctx)
        .await.unwrap();
    assert!(r.content.contains(":5:epsilon"));
}

#[tokio::test]
async fn context_separator_between_groups() {
    let tmp = tempfile::tempdir().unwrap();
    // Matches at line 1 (alpha) and line 5 (epsilon); -A=0 -B=0 but -C=0 → no context
    // Use -C=1 so groups don't merge: ranges [0,2] and [4,4+1] → gap at line 3
    make_file(tmp.path(), "f.txt", "aaa\nbbb\nccc\nddd\neee");
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(json!({"pattern": "aaa|eee", "output_mode": "content", "-C": 1}), &ctx)
        .await.unwrap();
    // Two groups with gap → should have -- separator
    assert!(r.content.contains("--"), "groups should be separated by --");
}

#[tokio::test]
async fn context_zero_has_no_effect() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = GrepTool;
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "gamma", "output_mode": "content", "-A": 0, "-B": 0}),
            &ctx,
        )
        .await.unwrap();
    let lines: Vec<_> = r.content.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains(":3:gamma"));
}
