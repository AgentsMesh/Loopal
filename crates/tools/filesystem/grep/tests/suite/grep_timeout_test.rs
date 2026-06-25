use loopal_tool_api::backend_types::{FileMatchResult, GrepSearchResult, MatchGroup, MatchLine};
use loopal_tool_grep::grep_format::{FormatOptions, OutputMode, format_results};

fn timed_out_empty() -> GrepSearchResult {
    GrepSearchResult {
        file_matches: vec![],
        total_match_count: 0,
        timed_out: true,
        overflow_path: None,
    }
}

fn timed_out_with_match() -> GrepSearchResult {
    GrepSearchResult {
        file_matches: vec![FileMatchResult {
            path: "x.rs".to_string(),
            groups: vec![MatchGroup {
                lines: vec![MatchLine {
                    line_num: 1,
                    content: "fn main() {}".to_string(),
                    is_match: true,
                }],
            }],
        }],
        total_match_count: 1,
        timed_out: true,
        overflow_path: None,
    }
}

#[test]
fn timeout_notice_replaces_no_matches_message() {
    let out = format_results(
        &timed_out_empty(),
        OutputMode::FilesWithMatches,
        50,
        500,
        &FormatOptions::default(),
    );
    assert!(out.contains("timed out"));
}

#[test]
fn timeout_notice_appended_after_matches() {
    let out = format_results(
        &timed_out_with_match(),
        OutputMode::Content,
        50,
        500,
        &FormatOptions::default(),
    );
    assert!(out.contains("x.rs"));
    assert!(out.contains("timed out"));
}

#[test]
fn no_timeout_notice_when_not_timed_out() {
    let mut r = timed_out_with_match();
    r.timed_out = false;
    let out = format_results(&r, OutputMode::Content, 50, 500, &FormatOptions::default());
    assert!(!out.contains("timed out"));
}
