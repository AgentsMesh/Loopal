//! Serialize structured search results to human-readable strings for overflow files.

use std::fmt::Write;

use loopal_tool_api::backend_types::{FileMatchResult, GlobEntry};

/// Serialize grep results to a human-readable `file:line:content` format.
pub fn serialize_grep_results(matches: &[FileMatchResult]) -> String {
    let mut out = String::new();
    for fm in matches {
        for group in &fm.groups {
            for line in &group.lines {
                let sep = if line.is_match { ':' } else { '-' };
                writeln!(
                    out,
                    "{}{sep}{}{sep}{}",
                    fm.path, line.line_num, line.content
                )
                .unwrap();
            }
        }
    }
    out
}

/// Serialize glob entries to a newline-delimited path list.
pub fn serialize_glob_results(entries: &[GlobEntry]) -> String {
    let mut out = String::with_capacity(entries.len() * 80);
    for entry in entries {
        out.push_str(&entry.path);
        out.push('\n');
    }
    out
}
