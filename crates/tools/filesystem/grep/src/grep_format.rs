use std::fmt::Write;

use loopal_error::LoopalError;
use loopal_tool_api::backend_types::{GrepSearchResult, MatchLine};

use crate::grep_format_summary::{format_count, format_files};

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
        Self {
            show_line_numbers: true,
            offset: 0,
            has_context: false,
        }
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
        write!(
            output,
            "\n... (search stopped at {max_total_matches} matches)"
        )
        .unwrap();
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
        write!(
            output,
            "\n\n(Showing {limit} of {available} matches. Use offset={next} to see more.)"
        )
        .unwrap();
    }
}
