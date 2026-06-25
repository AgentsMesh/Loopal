use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{FileMatchResult, GrepOptions, GrepSearchResult};
use loopal_tool_api::save_to_overflow_file;

use crate::limits::ResourceLimits;
use crate::search::{binary, grep_match, overflow_fmt};

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_one_file(
    path: &Path,
    re: &regex::Regex,
    multiline: bool,
    ctx_before: usize,
    ctx_after: usize,
    max: usize,
    total: &AtomicUsize,
    done: &AtomicBool,
) -> Option<FileMatchResult> {
    if binary::is_likely_binary(path) {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let indices = grep_match::find_match_indices(&content, &lines, re, multiline);
    if indices.is_empty() {
        return None;
    }
    let prev = total.fetch_add(indices.len(), Ordering::Relaxed);
    if prev + indices.len() >= max {
        done.store(true, Ordering::Relaxed);
    }
    let groups = grep_match::collect_context_groups(&lines, &indices, ctx_before, ctx_after);
    Some(FileMatchResult {
        path: path.to_string_lossy().into_owned(),
        groups,
    })
}

pub(crate) fn empty_result() -> GrepSearchResult {
    GrepSearchResult {
        file_matches: Vec::new(),
        total_match_count: 0,
        timed_out: false,
        overflow_path: None,
    }
}

pub(crate) fn maybe_save_overflow(truncated: bool, matches: &[FileMatchResult]) -> Option<String> {
    if truncated {
        Some(save_to_overflow_file(
            &overflow_fmt::serialize_grep_results(matches),
            "grep_results",
        ))
    } else {
        None
    }
}

pub(crate) fn search_single_file(
    opts: &GrepOptions,
    path: &Path,
    limits: &ResourceLimits,
) -> Result<GrepSearchResult, ToolIoError> {
    if binary::is_likely_binary(path) {
        return Ok(empty_result());
    }
    let re = super::grep::build_regex(opts)?;
    let Ok(content) = std::fs::read_to_string(path) else {
        return Ok(empty_result());
    };
    let lines: Vec<&str> = content.lines().collect();
    let indices = grep_match::find_match_indices(&content, &lines, &re, opts.multiline);
    if indices.is_empty() {
        return Ok(empty_result());
    }
    let truncated = indices.len() > limits.max_grep_matches;
    let count = indices.len().min(limits.max_grep_matches);
    let groups = grep_match::collect_context_groups(
        &lines,
        &indices,
        opts.context_before,
        opts.context_after,
    );
    let file_matches = vec![FileMatchResult {
        path: path.to_string_lossy().into_owned(),
        groups,
    }];
    let overflow_path = maybe_save_overflow(truncated, &file_matches);
    Ok(GrepSearchResult {
        total_match_count: count,
        timed_out: false,
        file_matches,
        overflow_path,
    })
}
