use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{FileMatchResult, GrepOptions, GrepSearchResult};

use crate::limits::ResourceLimits;
use crate::search::{binary, grep_match};

#[allow(clippy::too_many_arguments)]
pub(crate) fn search_one_file(
    path: &Path,
    re: &regex::Regex,
    multiline: bool,
    ctx_before: usize,
    ctx_after: usize,
    max: usize,
    max_file_bytes: u64,
    total: &AtomicUsize,
    done: &AtomicBool,
) -> Option<FileMatchResult> {
    if binary::is_likely_binary(path) {
        return None;
    }
    let content = read_text(path, max_file_bytes)?;
    let lines: Vec<&str> = content.lines().collect();
    let indices = grep_match::find_match_indices(&content, &lines, re, multiline);
    if indices.is_empty() {
        return None;
    }
    let prev = total.fetch_add(indices.len(), Ordering::Relaxed);
    if prev + indices.len() >= max {
        done.store(true, Ordering::Relaxed);
    }
    let remaining = max.saturating_sub(prev);
    if remaining == 0 {
        return None;
    }
    let indices = indices.into_iter().take(remaining).collect();
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
    let Some(content) = read_text(path, limits.max_file_read_bytes) else {
        return Ok(empty_result());
    };
    let lines: Vec<&str> = content.lines().collect();
    let indices = grep_match::find_match_indices(&content, &lines, &re, opts.multiline);
    if indices.is_empty() {
        return Ok(empty_result());
    }
    let max = opts.max_matches.min(limits.max_grep_matches).max(1);
    let count = indices.len().min(max);
    let limited: BTreeSet<_> = indices.iter().copied().take(count).collect();
    let groups = grep_match::collect_context_groups(
        &lines,
        &limited,
        opts.context_before,
        opts.context_after,
    );
    let file_matches = vec![FileMatchResult {
        path: path.to_string_lossy().into_owned(),
        groups,
    }];
    Ok(GrepSearchResult {
        total_match_count: count,
        timed_out: false,
        file_matches,
    })
}

fn read_text(path: &Path, max_bytes: u64) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > max_bytes {
        return None;
    }
    String::from_utf8(bytes).ok()
}
