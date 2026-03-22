use loopal_tool_api::{Tool, ToolContext};
use loopal_tool_ls::LsTool;
use serde_json::json;

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext {
        session_id: "test".into(),
        shared: None,
        pending_cwd_switch: Default::default(), memory_channel: None,
        backend,
    }
}

// --- long mode ---

#[tokio::test]
async fn long_mode_shows_permissions_and_size() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("hello.txt"), "hello world").unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"long": true}), &ctx).await.unwrap();
    assert!(!r.is_error);
    // Should contain permission string, size, and filename
    assert!(r.content.contains("rw"));
    assert!(r.content.contains("hello.txt"));
    // 11 bytes -> "11B"
    assert!(r.content.contains("11B"));
}

#[tokio::test]
async fn long_mode_shows_directory_indicator() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("subdir")).unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"long": true}), &ctx).await.unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("subdir/"));
    assert!(r.content.contains('d'));
}

#[tokio::test]
async fn long_mode_shows_mtime() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("f.txt"), "x").unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"long": true}), &ctx).await.unwrap();
    // Mtime should be a date like "2026-03-20 14:30"
    assert!(r.content.contains("20"));
    assert!(r.content.contains(':'));
}

// --- all mode (dotfiles) ---

#[tokio::test]
async fn hidden_files_excluded_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".hidden"), "h").unwrap();
    std::fs::write(tmp.path().join("visible"), "v").unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({}), &ctx).await.unwrap();
    assert!(!r.content.contains(".hidden"));
    assert!(r.content.contains("visible"));
}

#[tokio::test]
async fn all_mode_shows_hidden_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".hidden"), "h").unwrap();
    std::fs::write(tmp.path().join("visible"), "v").unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"all": true}), &ctx).await.unwrap();
    assert!(r.content.contains(".hidden"));
    assert!(r.content.contains("visible"));
}

// --- single file stat ---

#[tokio::test]
async fn path_to_file_shows_stat() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("info.txt"), "twelve bytes").unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"path": "info.txt"}), &ctx).await.unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("File:"));
    assert!(r.content.contains("Type: regular file"));
    assert!(r.content.contains("Size: 12 bytes"));
    assert!(r.content.contains("Modified:"));
}

#[tokio::test]
async fn stat_directory_path() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("mydir");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("a.txt"), "a").unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    // Pointing at a directory should list it, not stat it
    let r = tool.execute(json!({"path": "mydir"}), &ctx).await.unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("a.txt"));
    assert!(!r.content.contains("File:"));
}

// --- combined modes ---

#[tokio::test]
async fn long_and_all_combined() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".config"), "cfg").unwrap();
    std::fs::write(tmp.path().join("main.rs"), "fn main(){}").unwrap();
    let tool = LsTool;
    let ctx = make_ctx(tmp.path());
    let r = tool.execute(json!({"long": true, "all": true}), &ctx).await.unwrap();
    assert!(r.content.contains(".config"));
    assert!(r.content.contains("main.rs"));
    // Both entries should have permission strings
    let lines: Vec<&str> = r.content.lines().collect();
    assert!(lines.len() >= 2);
    assert!(lines.iter().all(|l| l.contains("rw") || l.contains("r-")));
}
