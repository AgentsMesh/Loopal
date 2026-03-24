use std::fmt::Write;

use loopal_error::LoopalError;
use loopal_tool_api::backend_types::{GrepSearchResult, MatchLine};

/// Output format for grep results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Content,
    FilesWithMatches,
    Count,
}

impl OutputMode {
    pub fn from_str_opt(s: Option<&str>) -> Result<Self, LoopalError> {
        match s {
            None | Some("files_with_matches") => Ok(Self::FilesWithMatches),
            Some("content") => Ok(Self::Content),
            Some("count") => Ok(Self::Count),
            Some(other) => Err(LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                format!("Invalid output_mode: {other}. Use content, files_with_matches, or count"),
            ))),
        }
    }
}

/// Formatting options separate from search behavior.
pub struct FormatOptions {
    pub show_line_numbers: bool,
    pub offset: usize,
    pub has_context: bool,
}
impl Default for FormatOptions {
    fn default() -> Self {
        Self { show_line_numbers: true, offset: 0, has_context: false }
    }
}

/// Format results according to the requested output mode.
pub fn format_results(
    results: &GrepSearchResult,
    mode: OutputMode,
    head_limit: usize,
    max_total_matches: usize,
    fmt_opts: &FormatOptions,
) -> String {
    if results.file_matches.is_empty() {
        return "No matches found.".to_string();
    }

    let mut output = match mode {
        OutputMode::Content => format_content(results, head_limit, fmt_opts),
        OutputMode::FilesWithMatches => format_files(results, head_limit, fmt_opts.offset),
        OutputMode::Count => format_count(results, fmt_opts.offset),
    };

    if results.total_match_count >= max_total_matches {
        write!(output, "\n... (search stopped at {max_total_matches} matches)").unwrap();
    }
    output
}

fn format_content(results: &GrepSearchResult, head_limit: usize, opts: &FormatOptions) -> String {
    let mut output = String::new();
    let mut emitted = 0usize;
    let mut skipped = 0usize;
    let mut first_entry = true;

    for fm in &results.file_matches {
        for group in &fm.groups {
            let match_count = group.lines.iter().filter(|l| l.is_match).count();
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
                &fm.path,
                &group.lines,
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

    append_content_footer(&mut output, results.total_match_count, head_limit, opts.offset);
    output
}

fn format_group(
    output: &mut String,
    path: &str,
    lines: &[MatchLine],
    opts: &FormatOptions,
    emitted: &mut usize,
    skipped: &mut usize,
    head_limit: usize,
) {
    for line in lines {
        if line.is_match && *skipped < opts.offset {
            *skipped += 1;
            continue;
        }
        if !line.is_match && *skipped < opts.offset {
            continue;
        }
        if *emitted >= head_limit && line.is_match {
            break;
        }
        if *emitted > 0 || !output.is_empty() {
            output.push('\n');
        }
        let sep = if line.is_match { ':' } else { '-' };
        if opts.show_line_numbers {
            write!(output, "{path}{sep}{}{sep}{}", line.line_num, line.content).unwrap();
        } else {
            write!(output, "{path}{sep}{}", line.content).unwrap();
        }
        if line.is_match {
            *emitted += 1;
        }
    }
}

fn append_content_footer(output: &mut String, total: usize, limit: usize, offset: usize) {
    let available = total.saturating_sub(offset);
    if available > limit {
        let next = offset + limit;
        write!(output, "\n\n(Showing {limit} of {available} matches. Use offset={next} to see more.)").unwrap();
    }
}

fn count_matches(fm: &loopal_tool_api::backend_types::FileMatchResult) -> usize {
    fm.groups.iter().flat_map(|g| &g.lines).filter(|l| l.is_match).count()
}

fn format_files(results: &GrepSearchResult, head_limit: usize, offset: usize) -> String {
    let mut file_counts: Vec<(&str, usize)> = results
        .file_matches.iter()
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
        write!(output, "\n\n(Showing {head_limit} of {available} files. Use offset={next} to see more.)").unwrap();
    }
    output
}

fn format_count(results: &GrepSearchResult, offset: usize) -> String {
    let mut entries: Vec<(&str, usize)> = results
        .file_matches.iter()
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
