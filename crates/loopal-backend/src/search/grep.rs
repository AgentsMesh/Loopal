//! Parallel regex content search with context lines and binary detection.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use globset::Glob;
use ignore::WalkState;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{FileMatchResult, GrepOptions, GrepSearchResult};
use parking_lot::Mutex;
use regex::RegexBuilder;

use crate::limits::ResourceLimits;
use crate::search::{binary, grep_match, walker};

/// Build a compiled regex from `GrepOptions`.
fn build_regex(opts: &GrepOptions) -> Result<regex::Regex, ToolIoError> {
    let effective = if opts.fixed_strings {
        regex::escape(&opts.pattern)
    } else {
        opts.pattern.clone()
    };
    RegexBuilder::new(&effective)
        .case_insensitive(opts.case_insensitive)
        .multi_line(opts.multiline)
        .dot_matches_new_line(opts.multiline)
        .size_limit(1_000_000)
        .build()
        .map_err(|e| ToolIoError::Other(format!("invalid regex: {e}")))
}

/// Execute a parallel grep search across files.
pub fn grep_search(
    opts: &GrepOptions,
    cwd: &Path,
    limits: &ResourceLimits,
) -> Result<GrepSearchResult, ToolIoError> {
    let search_path = opts
        .path
        .as_ref()
        .map(|p| cwd.join(p))
        .unwrap_or_else(|| cwd.to_path_buf());

    if search_path.is_file() {
        return search_single_file(opts, &search_path, limits);
    }

    let re = build_regex(opts)?;
    let glob_matcher = match opts.glob_filter.as_deref() {
        Some(g) => {
            let glob = Glob::new(g)
                .map_err(|e| ToolIoError::Other(format!("invalid glob filter: {e}")))?;
            Some(glob.compile_matcher())
        }
        None => None,
    };

    let max = opts.max_matches.min(limits.max_grep_matches);
    let ctx_before = opts.context_before;
    let ctx_after = opts.context_after;
    let multiline = opts.multiline;
    let total = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let results: Arc<Mutex<Vec<FileMatchResult>>> = Arc::new(Mutex::new(Vec::new()));
    let search_path = Arc::new(search_path);
    let glob_matcher = Arc::new(glob_matcher);

    let Some(w) = walker::build_walker(&search_path, opts.type_filter.as_deref()) else {
        return Ok(GrepSearchResult {
            file_matches: Vec::new(),
            total_match_count: 0,
        });
    };
    w.build_parallel().run(|| {
        let re = re.clone();
        let glob_matcher = Arc::clone(&glob_matcher);
        let search_path = Arc::clone(&search_path);
        let total = Arc::clone(&total);
        let done = Arc::clone(&done);
        let results = Arc::clone(&results);
        Box::new(move |entry| {
            if done.load(Ordering::Relaxed) {
                return WalkState::Quit;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return WalkState::Continue;
            }
            let path = entry.into_path();
            if !matches_glob(&path, &search_path, glob_matcher.as_ref().as_ref()) {
                return WalkState::Continue;
            }
            if let Some(fm) = search_one_file(
                &path, &re, multiline, ctx_before, ctx_after, max, &total, &done,
            ) {
                results.lock().push(fm);
            }
            WalkState::Continue
        })
    });

    let file_matches = Arc::try_unwrap(results).unwrap().into_inner();
    Ok(GrepSearchResult {
        total_match_count: total.load(Ordering::Relaxed),
        file_matches,
    })
}

fn matches_glob(path: &Path, root: &Path, gm: Option<&globset::GlobMatcher>) -> bool {
    let Some(gm) = gm else { return true };
    let rel = path.strip_prefix(root).unwrap_or(path);
    gm.is_match(rel) || gm.is_match(path.file_name().unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
fn search_one_file(
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

fn search_single_file(
    opts: &GrepOptions,
    path: &Path,
    limits: &ResourceLimits,
) -> Result<GrepSearchResult, ToolIoError> {
    let empty = GrepSearchResult {
        file_matches: Vec::new(),
        total_match_count: 0,
    };
    if binary::is_likely_binary(path) {
        return Ok(empty);
    }
    let re = build_regex(opts)?;
    let Ok(content) = std::fs::read_to_string(path) else {
        return Ok(empty);
    };
    let lines: Vec<&str> = content.lines().collect();
    let indices = grep_match::find_match_indices(&content, &lines, &re, opts.multiline);
    if indices.is_empty() {
        return Ok(empty);
    }
    let count = indices.len().min(limits.max_grep_matches);
    let groups = grep_match::collect_context_groups(
        &lines,
        &indices,
        opts.context_before,
        opts.context_after,
    );
    Ok(GrepSearchResult {
        total_match_count: count,
        file_matches: vec![FileMatchResult {
            path: path.to_string_lossy().into_owned(),
            groups,
        }],
    })
}
