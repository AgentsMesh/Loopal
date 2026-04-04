//! Tests for session cwd-based query: latest_session_for_cwd, list_sessions_for_cwd, normalize_cwd.

use std::path::Path;

use loopal_storage::SessionStore;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// latest_session_for_cwd
// ---------------------------------------------------------------------------

#[test]
fn test_latest_for_cwd_returns_most_recently_updated() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let s1 = store.create_session(Path::new("/project"), "m1").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let s2 = store.create_session(Path::new("/project"), "m2").unwrap();

    let latest = store
        .latest_session_for_cwd(Path::new("/project"))
        .unwrap()
        .expect("should find a session");
    // s2 was created (and thus updated) more recently
    assert_eq!(latest.id, s2.id);

    // Update s1's updated_at to be newer
    let mut s1_loaded = store.load_session(&s1.id).unwrap();
    s1_loaded.updated_at = chrono::Utc::now();
    store.update_session(&s1_loaded).unwrap();

    let latest = store
        .latest_session_for_cwd(Path::new("/project"))
        .unwrap()
        .expect("should find a session");
    assert_eq!(latest.id, s1.id, "s1 now has a newer updated_at");
}

#[test]
fn test_latest_for_cwd_returns_none_when_no_match() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    store.create_session(Path::new("/other"), "m1").unwrap();

    let result = store.latest_session_for_cwd(Path::new("/project")).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_latest_for_cwd_returns_none_when_empty() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let result = store.latest_session_for_cwd(Path::new("/project")).unwrap();
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// list_sessions_for_cwd
// ---------------------------------------------------------------------------

#[test]
fn test_list_for_cwd_filters_by_directory() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    store.create_session(Path::new("/alpha"), "m1").unwrap();
    store.create_session(Path::new("/alpha"), "m2").unwrap();
    store.create_session(Path::new("/beta"), "m3").unwrap();

    let alpha = store.list_sessions_for_cwd(Path::new("/alpha")).unwrap();
    assert_eq!(alpha.len(), 2);

    let beta = store.list_sessions_for_cwd(Path::new("/beta")).unwrap();
    assert_eq!(beta.len(), 1);
    assert_eq!(beta[0].model, "m3");
}

#[test]
fn test_list_for_cwd_sorted_by_updated_at() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let s1 = store.create_session(Path::new("/proj"), "m1").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let s2 = store.create_session(Path::new("/proj"), "m2").unwrap();

    let sessions = store.list_sessions_for_cwd(Path::new("/proj")).unwrap();
    assert_eq!(sessions.len(), 2);
    // Newest updated_at first
    assert_eq!(sessions[0].id, s2.id);
    assert_eq!(sessions[1].id, s1.id);
}

#[test]
fn test_list_for_cwd_empty_when_no_match() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    store.create_session(Path::new("/other"), "m1").unwrap();

    let result = store.list_sessions_for_cwd(Path::new("/proj")).unwrap();
    assert!(result.is_empty());
}

// ---------------------------------------------------------------------------
// normalize_cwd (tested via create + query round-trip)
// ---------------------------------------------------------------------------

#[test]
fn test_cwd_normalization_via_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    // Create session with the canonical tmpdir path
    let canonical = std::fs::canonicalize(tmp.path()).unwrap();
    store.create_session(&canonical, "m1").unwrap();

    // Query with the original (possibly non-canonical) path — should still match
    let result = store.latest_session_for_cwd(tmp.path()).unwrap();
    assert!(
        result.is_some(),
        "canonical and non-canonical paths should match"
    );
}

#[test]
fn test_cwd_normalization_nonexistent_path_fallback() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    // Path that doesn't exist — canonicalize falls back to raw path
    let fake = Path::new("/nonexistent/path/abc");
    store.create_session(fake, "m1").unwrap();

    let result = store.latest_session_for_cwd(fake).unwrap();
    assert!(
        result.is_some(),
        "non-existent path should match by raw string"
    );
}
