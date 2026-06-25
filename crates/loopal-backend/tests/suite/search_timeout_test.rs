use std::time::Duration;

use loopal_backend::ResourceLimits;
use loopal_backend::search::{glob_search, glob_search_async, grep_search, grep_search_async};
use loopal_tool_api::backend_types::{GlobOptions, GrepOptions};

fn tmp_with_files() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.rs"), "fn alpha() {}").unwrap();
    std::fs::write(tmp.path().join("b.rs"), "fn beta() {}").unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("c.rs"), "fn gamma() {}").unwrap();
    tmp
}

fn limits_with_walk(walk: Duration) -> ResourceLimits {
    ResourceLimits {
        walk_timeout: walk,
        ..ResourceLimits::default()
    }
}

fn glob_opts(pattern: &str) -> GlobOptions {
    GlobOptions {
        pattern: pattern.to_string(),
        path: None,
        type_filter: None,
        max_results: 10_000,
    }
}

fn grep_opts(pattern: &str) -> GrepOptions {
    GrepOptions {
        pattern: pattern.to_string(),
        path: None,
        glob_filter: None,
        case_insensitive: false,
        multiline: false,
        fixed_strings: false,
        context_before: 0,
        context_after: 0,
        type_filter: None,
        max_matches: 500,
    }
}

#[test]
fn glob_search_times_out_with_zero_walk_budget() {
    let tmp = tmp_with_files();
    let res = glob_search(
        &glob_opts("**/*.rs"),
        tmp.path(),
        &limits_with_walk(Duration::ZERO),
    )
    .unwrap();
    assert!(res.timed_out);
    assert!(!res.truncated);
}

#[test]
fn glob_search_completes_within_normal_budget() {
    let tmp = tmp_with_files();
    let res = glob_search(
        &glob_opts("**/*.rs"),
        tmp.path(),
        &ResourceLimits::default(),
    )
    .unwrap();
    assert!(!res.timed_out);
    assert_eq!(res.entries.len(), 3);
}

#[test]
fn grep_search_times_out_with_zero_walk_budget() {
    let tmp = tmp_with_files();
    let res = grep_search(
        &grep_opts("fn"),
        tmp.path(),
        &limits_with_walk(Duration::ZERO),
    )
    .unwrap();
    assert!(res.timed_out);
}

#[test]
fn grep_search_completes_within_normal_budget() {
    let tmp = tmp_with_files();
    let res = grep_search(&grep_opts("alpha"), tmp.path(), &ResourceLimits::default()).unwrap();
    assert!(!res.timed_out);
    assert_eq!(res.total_match_count, 1);
}

#[tokio::test]
async fn glob_search_async_returns_partial_before_hard_backstop() {
    let tmp = tmp_with_files();
    let res = glob_search_async(
        glob_opts("**/*.rs"),
        tmp.path().to_path_buf(),
        limits_with_walk(Duration::ZERO),
    )
    .await
    .unwrap();
    assert!(res.timed_out);
}

#[tokio::test]
async fn grep_search_async_returns_partial_before_hard_backstop() {
    let tmp = tmp_with_files();
    let res = grep_search_async(
        grep_opts("fn"),
        tmp.path().to_path_buf(),
        limits_with_walk(Duration::ZERO),
    )
    .await
    .unwrap();
    assert!(res.timed_out);
}
