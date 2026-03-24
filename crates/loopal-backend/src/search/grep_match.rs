//! Match-finding and context-grouping helpers for grep search.

use std::collections::BTreeSet;

use loopal_tool_api::backend_types::{MatchGroup, MatchLine};

/// Find matching line indices. Uses a newline-offset table for O(log n)
/// byte-offset → line-number conversion in multiline mode.
pub fn find_match_indices(
    content: &str,
    lines: &[&str],
    re: &regex::Regex,
    multiline: bool,
) -> BTreeSet<usize> {
    let mut indices = BTreeSet::new();
    if multiline {
        let offsets: Vec<usize> = content
            .bytes()
            .enumerate()
            .filter(|(_, b)| *b == b'\n')
            .map(|(i, _)| i)
            .collect();
        for mat in re.find_iter(content) {
            let start = offsets.partition_point(|&o| o < mat.start());
            let end = offsets.partition_point(|&o| o < mat.end());
            for idx in start..=end.min(lines.len().saturating_sub(1)) {
                indices.insert(idx);
            }
        }
    } else {
        for (idx, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                indices.insert(idx);
            }
        }
    }
    indices
}

/// Merge overlapping context ranges and build `MatchGroup` results.
pub fn collect_context_groups(
    lines: &[&str],
    match_indices: &BTreeSet<usize>,
    before: usize,
    after: usize,
) -> Vec<MatchGroup> {
    let last = lines.len().saturating_sub(1);
    let ranges: Vec<(usize, usize)> = match_indices
        .iter()
        .map(|&i| (i.saturating_sub(before), (i + after).min(last)))
        .collect();

    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in ranges {
        if let Some(prev) = merged.last_mut() {
            if r.0 <= prev.1 + 1 {
                prev.1 = prev.1.max(r.1);
                continue;
            }
        }
        merged.push(r);
    }

    merged
        .iter()
        .map(|&(start, end)| MatchGroup {
            lines: (start..=end)
                .map(|i| MatchLine {
                    line_num: i + 1,
                    content: lines[i].to_string(),
                    is_match: match_indices.contains(&i),
                })
                .collect(),
        })
        .collect()
}
