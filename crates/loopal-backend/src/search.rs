//! Glob and grep search with result caps.

use std::path::{Path, PathBuf};

use globset::Glob;
use ignore::WalkBuilder;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::{GlobResult, GrepMatch, GrepResult};
use regex::RegexBuilder;

use crate::limits::ResourceLimits;

/// Glob pattern search from a base directory with result cap.
pub fn glob_search(
    pattern: &str,
    base: Option<&str>,
    cwd: &Path,
    limits: &ResourceLimits,
) -> Result<GlobResult, ToolIoError> {
    let search_path = base
        .map(|b| cwd.join(b))
        .unwrap_or_else(|| cwd.to_path_buf());
    let glob = Glob::new(pattern).map_err(|e| ToolIoError::Other(format!("invalid glob: {e}")))?;
    let matcher = glob.compile_matcher();

    let mut paths = Vec::new();
    let mut truncated = false;

    for entry in WalkBuilder::new(&search_path)
        .follow_links(true)
        .build()
        .flatten()
    {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let full = entry.into_path();
        let rel = full.strip_prefix(&search_path).unwrap_or(&full);
        if matcher.is_match(rel) {
            paths.push(full.to_string_lossy().into_owned());
            if paths.len() >= limits.max_glob_results {
                truncated = true;
                break;
            }
        }
    }

    Ok(GlobResult { paths, truncated })
}

/// Regex search over file contents with match cap.
pub fn grep_search(
    pattern: &str,
    search_path: &Path,
    glob_filter: Option<&str>,
    limits: &ResourceLimits,
) -> Result<GrepResult, ToolIoError> {
    let re = RegexBuilder::new(pattern)
        .size_limit(1_000_000)
        .build()
        .map_err(|e| ToolIoError::Other(format!("invalid regex: {e}")))?;

    let glob_matcher = glob_filter.and_then(|g| Glob::new(g).ok().map(|gb| gb.compile_matcher()));

    let entries = collect_files(search_path);
    let mut matches = Vec::new();
    let mut truncated = false;

    for path in entries {
        if let Some(ref gm) = glob_matcher
            && let Some(name) = path.file_name()
            && !gm.is_match(name)
        {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (idx, line) in content.lines().enumerate() {
            if re.is_match(line) {
                matches.push(GrepMatch {
                    path: path.to_string_lossy().into_owned(),
                    line_number: idx + 1,
                    content: line.to_string(),
                });
                if matches.len() >= limits.max_grep_matches {
                    truncated = true;
                    break;
                }
            }
        }
        if truncated {
            break;
        }
    }

    Ok(GrepResult { matches, truncated })
}

fn collect_files(search_path: &Path) -> Vec<PathBuf> {
    if search_path.is_file() {
        return vec![search_path.to_path_buf()];
    }
    WalkBuilder::new(search_path)
        .follow_links(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|ft| ft.is_file()))
        .map(|e| e.into_path())
        .collect()
}
