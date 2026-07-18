use std::path::PathBuf;

use loopal_tool_api::FileReadTracker;
use loopal_tool_api::path::ResolvedPath;

fn rp(p: &str) -> ResolvedPath {
    ResolvedPath::from_backend_resolved(PathBuf::from(p))
}

#[test]
fn unrecorded_path_is_never_stale() {
    let t = FileReadTracker::new();
    assert!(!t.is_stale(&rp("/a.txt"), "anything"));
}

#[test]
fn unchanged_content_is_not_stale() {
    let t = FileReadTracker::new();
    t.record(&rp("/a.txt"), "hello");
    assert!(!t.is_stale(&rp("/a.txt"), "hello"));
}

#[test]
fn changed_content_is_stale() {
    let t = FileReadTracker::new();
    t.record(&rp("/a.txt"), "hello");
    assert!(t.is_stale(&rp("/a.txt"), "hello world"));
}

#[test]
fn re_record_clears_staleness() {
    let t = FileReadTracker::new();
    t.record(&rp("/a.txt"), "v1");
    t.record(&rp("/a.txt"), "v2"); // e.g. after the agent's own write
    assert!(!t.is_stale(&rp("/a.txt"), "v2"));
}
