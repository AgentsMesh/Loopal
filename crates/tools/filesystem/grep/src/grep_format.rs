use std::fmt::Write;

use std::path::PathBuf;

use crate::grep_search::{GrepResults, MatchLine, OutputMode};

/// Formatting options separate from search behavior.
pub struct FormatOptions {
    pub show_line_numbers: bool,
    pub offset: usize,
    pub has_context: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            offset: 0,
            has_context: false,
        }
    }
}

/// Format results according to the requested output mode.
pub fn format_results(
    results: &GrepResults,
    mode: OutputMode,
    head_limit: usize,
    max_total_matches: usize,
    fmt_opts: &FormatOptions,
) -> String {
    if results.file_matches.is_empty() {
        return "No matches found.".to_string();
    }

    let output = match mode {
        OutputMode::Content => format_content(results, head_limit, fmt_opts),
        OutputMode::FilesWithMatches => format_files(results, head_limit, fmt_opts.offset),
        OutputMode::Count => format_count(results, fmt_opts.offset),
    };

    let mut output = output;
    if results.total_match_count >= max_total_matches {
        write!(
            output,
            "\n... (search stopped at {max_total_matches} matches)"
        )
        .unwrap();
    }
    output
}

fn format_content(results: &GrepResults, head_limit: usize, opts: &FormatOptions) -> String {
    let mut output = String::new();
    let mut emitted = 0usize;
    let mut skipped = 0usize;
    let mut first_entry = true;

    for (path, groups) in &results.file_matches {
        for group in groups {
            let match_count = group.iter().filter(|l| l.is_match).count();
            // offset applies to match lines, context lines travel with their match
            if skipped + match_count <= opts.offset {
                skipped += match_count;
                continue;
            }
            if emitted >= head_limit {
                break;
            }
            if !first_entry && opts.has_context {
                output.push_str("\n--\n");
            }
            first_entry = false;
            format_group(
                &mut output,
                path,
                group,
                opts,
                &mut emitted,
                &mut skipped,
                head_limit,
            );
        }
        if emitted >= head_limit {
            break;
        }
    }

    append_content_footer(
        &mut output,
        results.total_match_count,
        head_limit,
        opts.offset,
    );
    output
}

fn format_group(
    output: &mut String,
    path: &std::path::Path,
    group: &[MatchLine],
    opts: &FormatOptions,
    emitted: &mut usize,
    skipped: &mut usize,
    head_limit: usize,
) {
    for line in group {
        if line.is_match && *skipped < opts.offset {
            *skipped += 1;
            continue;
        }
        if !line.is_match && *skipped < opts.offset {
            continue; // skip context lines whose match was skipped
        }
        if *emitted >= head_limit && line.is_match {
            break;
        }
        if *emitted > 0 || !output.is_empty() {
            output.push('\n');
        }
        let sep = if line.is_match { ':' } else { '-' };
        if opts.show_line_numbers {
            write!(
                output,
                "{}{sep}{}{sep}{}",
                path.display(),
                line.line_num,
                line.content
            )
            .unwrap();
        } else {
            write!(output, "{}{sep}{}", path.display(), line.content).unwrap();
        }
        if line.is_match {
            *emitted += 1;
        }
    }
}

fn append_content_footer(output: &mut String, total: usize, head_limit: usize, offset: usize) {
    let available = total.saturating_sub(offset);
    if available > head_limit {
        let next_offset = offset + head_limit;
        write!(
            output,
            "\n\n(Showing {head_limit} of {available} matches. Use offset={next_offset} to see more.)"
        )
        .unwrap();
    }
}

fn format_files(results: &GrepResults, head_limit: usize, offset: usize) -> String {
    let mut file_counts: Vec<(&PathBuf, usize)> = results
        .file_matches
        .iter()
        .map(|(p, groups)| {
            (
                p,
                groups
                    .iter()
                    .flat_map(|g| g.iter())
                    .filter(|l| l.is_match)
                    .count(),
            )
        })
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
        write!(output, "{}: {} matches", path.display(), count).unwrap();
    }

    let available = total_files.saturating_sub(offset);
    if available > head_limit {
        let next_offset = offset + head_limit;
        write!(
            output,
            "\n\n(Showing {head_limit} of {available} files. Use offset={next_offset} to see more.)"
        )
        .unwrap();
    }
    output
}

fn format_count(results: &GrepResults, offset: usize) -> String {
    let mut entries: Vec<(&PathBuf, usize)> = results
        .file_matches
        .iter()
        .map(|(p, groups)| {
            (
                p,
                groups
                    .iter()
                    .flat_map(|g| g.iter())
                    .filter(|l| l.is_match)
                    .count(),
            )
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    if offset == 0 {
        let file_count = entries.len();
        return format!(
            "{} matches across {} files",
            results.total_match_count, file_count
        );
    }
    let mut output = String::new();
    for (path, count) in entries.into_iter().skip(offset) {
        if !output.is_empty() {
            output.push('\n');
        }
        write!(output, "{}: {count}", path.display()).unwrap();
    }
    output
}
