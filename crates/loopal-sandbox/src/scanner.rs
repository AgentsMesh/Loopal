use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use walkdir::WalkDir;

use crate::sensitive_patterns::SENSITIVE_FILE_GLOBS;

/// Scan a directory tree for files matching sensitive patterns.
///
/// Returns a list of relative paths (from `root`) that match.
/// The scan is bounded to `max_depth` levels and `max_results` matches.
pub fn scan_sensitive_files(root: &Path, max_depth: usize, max_results: usize) -> Vec<String> {
    let glob_set = build_sensitive_glob_set();
    let mut results = Vec::new();

    for entry in WalkDir::new(root)
        .max_depth(max_depth)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if results.len() >= max_results {
            break;
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Match against the path relative to root
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let rel_str = rel.to_string_lossy();
        if glob_set.is_match(rel_str.as_ref()) {
            results.push(rel_str.into_owned());
        }
    }

    results
}

/// Build a GlobSet from the default sensitive file patterns.
fn build_sensitive_glob_set() -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in SENSITIVE_FILE_GLOBS {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}
