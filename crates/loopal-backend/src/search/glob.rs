//! Glob pattern search with file-type filtering and modification time.

use std::path::Path;
use std::time::UNIX_EPOCH;

use globset::Glob;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{GlobEntry, GlobOptions, GlobSearchResult};

use crate::limits::ResourceLimits;
use crate::search::walker;

/// Execute a glob search and return matching entries.
pub fn glob_search(
    opts: &GlobOptions,
    cwd: &Path,
    limits: &ResourceLimits,
) -> Result<GlobSearchResult, ToolIoError> {
    let search_path = opts
        .path
        .as_ref()
        .map(|p| cwd.join(p))
        .unwrap_or_else(|| cwd.to_path_buf());

    let glob =
        Glob::new(&opts.pattern).map_err(|e| ToolIoError::Other(format!("invalid glob: {e}")))?;
    let matcher = glob.compile_matcher();

    let max = opts.max_results.min(limits.max_glob_results);
    let Some(walker) = walker::build_walker(&search_path, opts.type_filter.as_deref()) else {
        return Ok(GlobSearchResult {
            entries: Vec::new(),
            truncated: false,
        });
    };

    let mut entries = Vec::new();
    let mut truncated = false;

    for entry in walker.build().flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        let rel = match path.strip_prefix(&search_path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !matcher.is_match(rel) {
            continue;
        }
        let modified_secs = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        entries.push(GlobEntry {
            path: path.to_string_lossy().into_owned(),
            modified_secs,
        });

        if entries.len() >= max {
            truncated = true;
            break;
        }
    }

    Ok(GlobSearchResult { entries, truncated })
}
