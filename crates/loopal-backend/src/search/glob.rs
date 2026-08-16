use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, UNIX_EPOCH};

use globset::Glob;
use ignore::WalkState;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{GlobEntry, GlobOptions, GlobSearchResult};
use parking_lot::Mutex;

use crate::limits::ResourceLimits;
use crate::search::walker;

pub fn glob_search(
    opts: &GlobOptions,
    cwd: &Path,
    limits: &ResourceLimits,
) -> Result<GlobSearchResult, ToolIoError> {
    let search_path = opts
        .path
        .as_ref()
        .map(|p| p.as_path().to_path_buf())
        .unwrap_or_else(|| cwd.to_path_buf());

    let glob =
        Glob::new(&opts.pattern).map_err(|e| ToolIoError::Other(format!("invalid glob: {e}")))?;
    let max = opts.max_results.min(limits.max_glob_results).max(1);

    let Some(w) = walker::build_walker(&search_path, opts.type_filter.as_deref()) else {
        return Ok(GlobSearchResult {
            entries: Vec::new(),
            truncated: false,
            timed_out: false,
        });
    };

    let deadline = Instant::now() + limits.walk_timeout;
    let done = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let entries: Arc<Mutex<Vec<GlobEntry>>> = Arc::new(Mutex::new(Vec::new()));
    let search_path = Arc::new(search_path);
    let matcher = Arc::new(glob.compile_matcher());

    w.build_parallel().run(|| {
        let done = Arc::clone(&done);
        let timed_out = Arc::clone(&timed_out);
        let entries = Arc::clone(&entries);
        let search_path = Arc::clone(&search_path);
        let matcher = Arc::clone(&matcher);
        Box::new(move |entry| {
            if done.load(Ordering::Relaxed) {
                return WalkState::Quit;
            }
            if Instant::now() >= deadline {
                done.store(true, Ordering::Relaxed);
                timed_out.store(true, Ordering::Relaxed);
                return WalkState::Quit;
            }
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return WalkState::Continue;
            }
            let Ok(rel) = entry.path().strip_prefix(search_path.as_path()) else {
                return WalkState::Continue;
            };
            if !matcher.is_match(rel) {
                return WalkState::Continue;
            }
            let modified_secs = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            let n = {
                let mut guard = entries.lock();
                guard.push(GlobEntry {
                    path: entry.path().to_string_lossy().into_owned(),
                    modified_secs,
                });
                guard.len()
            };
            if n >= max {
                done.store(true, Ordering::Relaxed);
                return WalkState::Quit;
            }
            WalkState::Continue
        })
    });

    let entries = Arc::try_unwrap(entries).unwrap().into_inner();
    let truncated = entries.len() >= max;

    Ok(GlobSearchResult {
        entries,
        truncated,
        timed_out: timed_out.load(Ordering::Relaxed),
    })
}
