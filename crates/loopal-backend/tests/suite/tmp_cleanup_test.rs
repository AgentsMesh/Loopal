use std::collections::HashSet;
use std::path::{Path, PathBuf};

use loopal_backend::tmp_cleanup::is_valid_session_id;
use loopal_backend::{
    cleanup_orphans_in, cleanup_session_tmp, cleanup_session_tmp_in, create_log_file,
    create_log_file_in, loopal_tmp_root, session_bash_dir, session_bash_dir_in, session_tmp_root,
    session_tmp_root_in,
};
use tempfile::TempDir;

fn unique_session_id() -> String {
    format!("test-{}", uuid::Uuid::new_v4().simple())
}

async fn make_log_file(session_id: &str) -> PathBuf {
    let (p, _w) = create_log_file(session_id).await.expect("create log file");
    p
}

// reason: cleanup_orphans tests must NOT scan loopal_tmp_root or they will
// delete sibling tests' session dirs created after the live snapshot.
// Materialise an isolated root with handcrafted session subdirs and use
// cleanup_orphans_in so the blast radius is contained to this test.
async fn make_isolated_session(root: &Path, session_id: &str) -> PathBuf {
    let bash = root.join(session_id).join("bash");
    tokio::fs::create_dir_all(&bash).await.unwrap();
    let log = bash.join(format!("{}.log", uuid::Uuid::new_v4().simple()));
    tokio::fs::write(&log, b"").await.unwrap();
    log
}

#[test]
fn is_valid_session_id_rejects_path_traversal() {
    assert!(!is_valid_session_id(""));
    assert!(!is_valid_session_id("."));
    assert!(!is_valid_session_id(".."));
    assert!(!is_valid_session_id("a/b"));
    assert!(!is_valid_session_id("a\\b"));
    assert!(!is_valid_session_id("a\0b"));
    assert!(!is_valid_session_id("a\nb"));
    assert!(!is_valid_session_id("a\tb"));
    assert!(is_valid_session_id("abc"));
    assert!(is_valid_session_id("session-uuid-1234"));
}

#[tokio::test]
async fn create_log_file_rejects_invalid_session_id() {
    let err = create_log_file("..").await.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("invalid session id"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn cleanup_session_tmp_removes_entire_dir_with_empty_exclude() {
    let sid = unique_session_id();
    let log_path = make_log_file(&sid).await;
    assert!(log_path.exists());

    cleanup_session_tmp(&sid, &[]).await;

    assert!(
        !session_tmp_root(&sid).exists(),
        "session root must be gone"
    );
}

#[tokio::test]
async fn cleanup_session_tmp_keeps_excluded_paths() {
    let sid = unique_session_id();
    let keep = make_log_file(&sid).await;
    let kill = make_log_file(&sid).await;
    let bash_dir = session_bash_dir(&sid);

    cleanup_session_tmp(&sid, std::slice::from_ref(&keep)).await;

    assert!(keep.exists(), "excluded path must survive");
    assert!(!kill.exists(), "non-excluded path must be removed");
    // dir may or may not still exist depending on whether the kept file
    // was the only entry; what matters is the contract above. Cleanup so
    // we don't leak the temp dir across test runs.
    let _ = tokio::fs::remove_file(&keep).await;
    let _ = tokio::fs::remove_dir(&bash_dir).await;
    let _ = tokio::fs::remove_dir(session_tmp_root(&sid)).await;
}

#[tokio::test]
async fn cleanup_session_tmp_skips_invalid_session_id() {
    // reason: must NOT touch loopal_tmp_root for traversal ids.
    let root = loopal_tmp_root();
    let marker_session = unique_session_id();
    let _ = make_log_file(&marker_session).await; // ensure root exists
    let marker_path = session_tmp_root(&marker_session);
    assert!(marker_path.exists());

    cleanup_session_tmp("..", &[]).await;
    assert!(root.exists(), "loopal root must be untouched");
    assert!(marker_path.exists(), "other session must be untouched");

    cleanup_session_tmp(&marker_session, &[]).await;
}

#[tokio::test]
async fn cleanup_orphans_removes_dirs_not_in_live_sessions() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();

    let orphan = unique_session_id();
    let alive = unique_session_id();
    let _ = make_isolated_session(&root, &orphan).await;
    let _ = make_isolated_session(&root, &alive).await;

    let mut live: HashSet<String> = HashSet::new();
    live.insert(alive.clone());

    cleanup_orphans_in(&root, &live).await;

    assert!(!root.join(&orphan).exists(), "orphan dir must be removed");
    assert!(root.join(&alive).exists(), "live session dir must survive");
}

#[tokio::test]
async fn cleanup_orphans_handles_missing_root() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("does-not-exist");
    let live: HashSet<String> = HashSet::new();
    cleanup_orphans_in(&missing, &live).await;
    assert!(!missing.exists());
}

#[tokio::test]
async fn create_log_file_in_writes_to_explicit_root() {
    let tmp = TempDir::new().unwrap();
    let sid = unique_session_id();
    let (p, _w) = create_log_file_in(tmp.path(), &sid).await.unwrap();
    assert!(
        p.starts_with(tmp.path()),
        "log file must live under explicit root"
    );
    assert!(p.exists());
}

#[tokio::test]
async fn cleanup_session_tmp_in_removes_only_inside_explicit_root() {
    let tmp = TempDir::new().unwrap();
    let sid = unique_session_id();
    let (p, _w) = create_log_file_in(tmp.path(), &sid).await.unwrap();
    assert!(p.exists());

    cleanup_session_tmp_in(tmp.path(), &sid, &[]).await;

    assert!(
        !session_tmp_root_in(tmp.path(), &sid).exists(),
        "session root inside explicit root must be gone"
    );
}

#[test]
fn session_path_helpers_compose_root_with_session_and_bash() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        session_tmp_root_in(tmp.path(), "sid"),
        tmp.path().join("sid")
    );
    assert_eq!(
        session_bash_dir_in(tmp.path(), "sid"),
        tmp.path().join("sid").join("bash")
    );
}
