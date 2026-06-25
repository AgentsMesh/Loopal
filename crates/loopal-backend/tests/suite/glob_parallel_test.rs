use loopal_backend::ResourceLimits;
use loopal_backend::search::glob_search;
use loopal_tool_api::backend_types::GlobOptions;

fn glob_opts(pattern: &str) -> GlobOptions {
    GlobOptions {
        pattern: pattern.to_string(),
        path: None,
        type_filter: None,
        max_results: 10_000,
    }
}

#[test]
fn parallel_glob_finds_all_nested_matches() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.rs"), "").unwrap();
    let sub = tmp.path().join("sub/deep");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(tmp.path().join("sub/b.rs"), "").unwrap();
    std::fs::write(sub.join("c.rs"), "").unwrap();
    std::fs::write(tmp.path().join("skip.txt"), "").unwrap();

    let res = glob_search(
        &glob_opts("**/*.rs"),
        tmp.path(),
        &ResourceLimits::default(),
    )
    .unwrap();

    let paths: Vec<&str> = res.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(res.entries.len(), 3);
    assert!(paths.iter().any(|p| p.ends_with("a.rs")));
    assert!(paths.iter().any(|p| p.ends_with("b.rs")));
    assert!(paths.iter().any(|p| p.ends_with("c.rs")));
    assert!(!res.truncated);
    assert!(!res.timed_out);
}

#[test]
fn parallel_glob_truncates_at_max_results_with_tolerance() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..50 {
        std::fs::write(tmp.path().join(format!("f{i:02}.rs")), "").unwrap();
    }
    let limits = ResourceLimits {
        max_glob_results: 10,
        ..ResourceLimits::default()
    };

    let res = glob_search(&glob_opts("**/*.rs"), tmp.path(), &limits).unwrap();

    assert!(res.truncated);
    assert!(!res.timed_out);
    assert!(res.entries.len() >= 10);
    assert!(res.entries.len() < 50);
}

#[cfg(unix)]
#[test]
fn glob_does_not_follow_symlinks() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    let outside = tmp.path().join("outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(root.join("real.rs"), "").unwrap();
    std::fs::write(outside.join("secret.rs"), "").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

    let res = glob_search(&glob_opts("**/*.rs"), &root, &ResourceLimits::default()).unwrap();

    let paths: Vec<&str> = res.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.iter().any(|p| p.ends_with("real.rs")));
    assert!(!paths.iter().any(|p| p.ends_with("secret.rs")));
}
