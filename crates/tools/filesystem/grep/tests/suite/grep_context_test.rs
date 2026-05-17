use loopal_tool_api::{Tool, ToolContext, TypedBridge};
use loopal_tool_grep::{GrepParams, GrepTool};
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        Default::default(),
        "test-session",
    );
    ToolContext::new(backend, "test")
}

fn make_file(dir: &std::path::Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).unwrap();
}

fn make_tool() -> TypedBridge<GrepTool, GrepParams> {
    TypedBridge::new(GrepTool)
}

const FIVE_LINES: &str = "alpha\nbeta\ngamma\ndelta\nepsilon";

#[tokio::test]
async fn context_after_shows_lines_after_match() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "gamma", "output_mode": "content", "-A": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains(":3:gamma"), "match line");
    assert!(r.content.contains("-4-delta"), "context after");
    assert!(!r.content.contains("epsilon"));
}

#[tokio::test]
async fn context_before_shows_lines_before_match() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "gamma", "output_mode": "content", "-B": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains("-2-beta"), "context before");
    assert!(r.content.contains(":3:gamma"), "match line");
    assert!(!r.content.contains("alpha"));
}

#[tokio::test]
async fn context_c_sets_both_directions() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "gamma", "output_mode": "content", "-C": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains("-2-beta"));
    assert!(r.content.contains(":3:gamma"));
    assert!(r.content.contains("-4-delta"));
}

#[tokio::test]
async fn context_merges_overlapping_ranges() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "beta|delta", "output_mode": "content", "-C": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        !r.content.contains("--"),
        "ranges should merge, no separator"
    );
    assert!(r.content.contains("alpha"));
    assert!(r.content.contains("epsilon"));
}

#[tokio::test]
async fn context_at_file_boundary() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());

    let r = tool
        .execute(
            json!({"pattern": "alpha", "output_mode": "content", "-B": 5}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains(":1:alpha"));

    let r = tool
        .execute(
            json!({"pattern": "epsilon", "output_mode": "content", "-A": 5}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains(":5:epsilon"));
}

#[tokio::test]
async fn context_separator_between_groups() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", "aaa\nbbb\nccc\nddd\neee");
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "aaa|eee", "output_mode": "content", "-C": 1}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(r.content.contains("--"), "groups should be separated by --");
}

#[tokio::test]
async fn context_zero_has_no_effect() {
    let tmp = tempfile::tempdir().unwrap();
    make_file(tmp.path(), "f.txt", FIVE_LINES);
    let tool = make_tool();
    let ctx = make_ctx(tmp.path());
    let r = tool
        .execute(
            json!({"pattern": "gamma", "output_mode": "content", "-A": 0, "-B": 0}),
            &ctx,
        )
        .await
        .unwrap();
    let lines: Vec<_> = r.content.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains(":3:gamma"));
}
