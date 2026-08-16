use loopal_backend::ResourceLimits;
use loopal_backend::search::{glob_search, grep_search};
use loopal_tool_api::ResolvedPath;
use loopal_tool_api::backend_types::{GlobOptions, GrepOptions};

fn grep_opts(pattern: &str, path: &std::path::Path) -> GrepOptions {
    GrepOptions {
        pattern: pattern.into(),
        path: Some(ResolvedPath::from_backend_resolved(path.to_path_buf())),
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
fn unknown_glob_type_returns_empty_result() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("input.rs"), "fn main() {}").unwrap();
    let options = GlobOptions {
        pattern: "**/*.rs".into(),
        path: None,
        type_filter: Some("__not_a_real_type__".into()),
        max_results: 100,
    };

    let result = glob_search(&options, tmp.path(), &ResourceLimits::default()).unwrap();

    assert!(result.entries.is_empty());
    assert!(!result.truncated);
    assert!(!result.timed_out);
}

#[test]
fn fixed_string_single_file_matches_literal_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("input.txt");
    std::fs::write(&path, "a.b\naxb\n").unwrap();
    let mut options = grep_opts("a.b", &path);
    options.fixed_strings = true;

    let result = grep_search(&options, tmp.path(), &ResourceLimits::default()).unwrap();

    assert_eq!(result.total_match_count, 1);
    assert_eq!(result.file_matches.len(), 1);
}

#[test]
fn single_file_no_match_returns_empty_result() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("input.txt");
    std::fs::write(&path, "haystack\n").unwrap();

    let result = grep_search(
        &grep_opts("needle", &path),
        tmp.path(),
        &ResourceLimits::default(),
    )
    .unwrap();

    assert_eq!(result.total_match_count, 0);
    assert!(result.file_matches.is_empty());
    assert!(!result.timed_out);
}

#[test]
fn single_file_over_read_limit_returns_empty_result() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("input.txt");
    std::fs::write(&path, "needle beyond limit").unwrap();
    let limits = ResourceLimits {
        max_file_read_bytes: 4,
        ..ResourceLimits::default()
    };

    let result = grep_search(&grep_opts("needle", &path), tmp.path(), &limits).unwrap();

    assert_eq!(result.total_match_count, 0);
    assert!(result.file_matches.is_empty());
}

#[test]
fn directory_glob_filter_skips_nonmatching_files() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("keep.rs"), "needle\n").unwrap();
    std::fs::write(tmp.path().join("skip.txt"), "needle\n").unwrap();
    let mut options = grep_opts("needle", tmp.path());
    options.glob_filter = Some("*.rs".into());

    let result = grep_search(&options, tmp.path(), &ResourceLimits::default()).unwrap();

    assert_eq!(result.total_match_count, 1);
    assert_eq!(result.file_matches.len(), 1);
    assert!(result.file_matches[0].path.ends_with("keep.rs"));
}

#[test]
fn unknown_grep_type_returns_empty_result() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("input.rs"), "needle\n").unwrap();
    let mut options = grep_opts("needle", tmp.path());
    options.type_filter = Some("__not_a_real_type__".into());

    let result = grep_search(&options, tmp.path(), &ResourceLimits::default()).unwrap();

    assert_eq!(result.total_match_count, 0);
    assert!(result.file_matches.is_empty());
}

#[test]
fn binary_files_are_skipped_for_single_file_and_directory_searches() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("binary.dat");
    std::fs::write(&path, b"needle\0hidden\n").unwrap();

    let single = grep_search(
        &grep_opts("needle", &path),
        tmp.path(),
        &ResourceLimits::default(),
    )
    .unwrap();
    assert_eq!(single.total_match_count, 0);

    let directory = grep_search(
        &grep_opts("needle", tmp.path()),
        tmp.path(),
        &ResourceLimits::default(),
    )
    .unwrap();
    assert_eq!(directory.total_match_count, 0);
    assert!(directory.file_matches.is_empty());
}
