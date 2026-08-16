use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use globset::Glob;
use ignore::WalkState;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{FileMatchResult, GrepOptions, GrepSearchResult};
use parking_lot::Mutex;
use regex::RegexBuilder;

use crate::limits::ResourceLimits;
use crate::search::grep_file::{empty_result, search_one_file, search_single_file};
use crate::search::walker;

pub(crate) fn build_regex(opts: &GrepOptions) -> Result<regex::Regex, ToolIoError> {
    let pat = if opts.fixed_strings {
        regex::escape(&opts.pattern)
    } else {
        opts.pattern.clone()
    };
    RegexBuilder::new(&pat)
        .case_insensitive(opts.case_insensitive)
        .multi_line(opts.multiline)
        .dot_matches_new_line(opts.multiline)
        .size_limit(1_000_000)
        .build()
        .map_err(|e| ToolIoError::Other(format!("invalid regex: {e}")))
}

pub fn grep_search(
    opts: &GrepOptions,
    cwd: &Path,
    limits: &ResourceLimits,
) -> Result<GrepSearchResult, ToolIoError> {
    let search_path = opts
        .path
        .as_ref()
        .map(|p| p.as_path().to_path_buf())
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

    let max = opts.max_matches.min(limits.max_grep_matches).max(1);
    let ctx_before = opts.context_before;
    let ctx_after = opts.context_after;
    let multiline = opts.multiline;
    let max_file_bytes = limits.max_file_read_bytes;
    let total = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + limits.walk_timeout;
    let results: Arc<Mutex<Vec<FileMatchResult>>> = Arc::new(Mutex::new(Vec::new()));
    let search_path = Arc::new(search_path);
    let glob_matcher = Arc::new(glob_matcher);

    let Some(w) = walker::build_walker(&search_path, opts.type_filter.as_deref()) else {
        return Ok(empty_result());
    };
    w.build_parallel().run(|| {
        let re = re.clone();
        let glob_matcher = Arc::clone(&glob_matcher);
        let search_path = Arc::clone(&search_path);
        let total = Arc::clone(&total);
        let done = Arc::clone(&done);
        let timed_out = Arc::clone(&timed_out);
        let results = Arc::clone(&results);
        Box::new(move |entry| {
            if done.load(Ordering::Relaxed) {
                return WalkState::Quit;
            }
            if Instant::now() >= deadline {
                done.store(true, Ordering::Relaxed);
                timed_out.store(true, Ordering::Relaxed);
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
                &path,
                &re,
                multiline,
                ctx_before,
                ctx_after,
                max,
                max_file_bytes,
                &total,
                &done,
            ) {
                results.lock().push(fm);
            }
            WalkState::Continue
        })
    });

    let file_matches = Arc::try_unwrap(results).unwrap().into_inner();
    Ok(GrepSearchResult {
        total_match_count: total.load(Ordering::Relaxed),
        timed_out: timed_out.load(Ordering::Relaxed),
        file_matches,
    })
}

fn matches_glob(path: &Path, root: &Path, gm: Option<&globset::GlobMatcher>) -> bool {
    let Some(gm) = gm else { return true };
    let rel = path.strip_prefix(root).unwrap_or(path);
    gm.is_match(rel) || gm.is_match(path.file_name().unwrap_or_default())
}
