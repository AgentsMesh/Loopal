//! Summary output formats: files_with_matches and count modes.

use std::fmt::Write;

use loopal_tool_api::backend_types::{FileMatchResult, GrepSearchResult};

fn count_matches(fm: &FileMatchResult) -> usize {
    fm.groups
        .iter()
        .flat_map(|g| &g.lines)
        .filter(|l| l.is_match)
        .count()
}

pub fn format_files(results: &GrepSearchResult, head_limit: usize, offset: usize) -> String {
    let mut file_counts: Vec<(&str, usize)> = results
        .file_matches
        .iter()
        .map(|fm| (fm.path.as_str(), count_matches(fm)))
        .collect();
    file_counts.sort_by(|a, b| b.1.cmp(&a.1));

    let total_files = file_counts.len();
    let mut output = String::new();

    for (emitted, (path, count)) in file_counts.into_iter().skip(offset).enumerate() {
        if emitted >= head_limit {
            break;
        }
        if emitted > 0 {
            output.push('\n');
        }
        write!(output, "{path}: {count} matches").unwrap();
    }

    let available = total_files.saturating_sub(offset);
    if available > head_limit {
        let next = offset + head_limit;
        write!(
            output,
            "\n\n(Showing {head_limit} of {available} files. Use offset={next} to see more.)"
        )
        .unwrap();
    }
    output
}

pub fn format_count(results: &GrepSearchResult, offset: usize) -> String {
    let mut entries: Vec<(&str, usize)> = results
        .file_matches
        .iter()
        .map(|fm| (fm.path.as_str(), count_matches(fm)))
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    if offset == 0 {
        return format!(
            "{} matches across {} files",
            results.total_match_count,
            entries.len()
        );
    }
    let mut output = String::new();
    for (path, count) in entries.into_iter().skip(offset) {
        if !output.is_empty() {
            output.push('\n');
        }
        write!(output, "{path}: {count}").unwrap();
    }
    output
}
