use loopal_types::error::LoopalError;
use regex::Regex;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Output format for grep results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Show matching lines with file:line: prefix (classic grep output).
    Content,
    /// Show only file paths that contain matches.
    FilesWithMatches,
    /// Show only per-file match counts.
    Count,
}

impl OutputMode {
    pub fn from_str_opt(s: Option<&str>) -> Result<Self, LoopalError> {
        match s {
            None | Some("files_with_matches") => Ok(Self::FilesWithMatches),
            Some("content") => Ok(Self::Content),
            Some("count") => Ok(Self::Count),
            Some(other) => Err(LoopalError::Tool(
                loopal_types::error::ToolError::InvalidInput(format!(
                    "Invalid output_mode: {other}. Use content, files_with_matches, or count"
                )),
            )),
        }
    }
}

/// Collected grep results ready for formatting.
pub struct GrepResults {
    /// (file_path, Vec<(line_number, line_text)>)
    pub file_matches: Vec<(PathBuf, Vec<(usize, String)>)>,
    pub total_match_count: usize,
}

/// Walk files, apply glob filter, search with regex, collect results.
pub fn search_files(
    search_path: &PathBuf,
    re: &Regex,
    include_glob: Option<&globset::GlobMatcher>,
    max_total_matches: usize,
) -> GrepResults {
    let entries: Vec<PathBuf> = if search_path.is_file() {
        vec![search_path.clone()]
    } else {
        WalkDir::new(search_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect()
    };

    let mut file_matches = Vec::new();
    let mut total = 0;

    'outer: for path in entries {
        if let Some(glob_matcher) = include_glob
            && let Some(name) = path.file_name()
                && !glob_matcher.is_match(name) {
                    continue;
                }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut hits = Vec::new();
        for (line_num, line) in content.lines().enumerate() {
            if re.is_match(line) {
                hits.push((line_num + 1, line.to_string()));
                total += 1;
                if total >= max_total_matches {
                    file_matches.push((path, hits));
                    break 'outer;
                }
            }
        }
        if !hits.is_empty() {
            file_matches.push((path, hits));
        }
    }

    GrepResults { file_matches, total_match_count: total }
}

/// Format results according to the requested output mode.
pub fn format_results(
    results: &GrepResults,
    mode: OutputMode,
    head_limit: usize,
    max_total_matches: usize,
) -> String {
    if results.file_matches.is_empty() {
        return "No matches found.".to_string();
    }

    let mut output = String::new();
    let mut emitted = 0;

    match mode {
        OutputMode::Content => {
            for (path, hits) in &results.file_matches {
                for (line_num, line) in hits {
                    if emitted >= head_limit { break; }
                    if emitted > 0 { output.push('\n'); }
                    output.push_str(&format!("{}:{}:{}", path.display(), line_num, line));
                    emitted += 1;
                }
                if emitted >= head_limit { break; }
            }
            if results.total_match_count > head_limit {
                output.push_str(&format!(
                    "\n\n(Showing {head_limit} of {} matches. \
                     Use head_limit or narrow your pattern to see more.)",
                    results.total_match_count
                ));
            }
        }
        OutputMode::FilesWithMatches => {
            let mut file_counts: Vec<(&PathBuf, usize)> = results
                .file_matches.iter().map(|(p, h)| (p, h.len())).collect();
            file_counts.sort_by(|a, b| b.1.cmp(&a.1));
            for (path, count) in &file_counts {
                if emitted >= head_limit { break; }
                if emitted > 0 { output.push('\n'); }
                output.push_str(&format!("{}: {} matches", path.display(), count));
                emitted += 1;
            }
            if results.file_matches.len() > head_limit {
                output.push_str(&format!(
                    "\n\n(Showing {head_limit} of {} files.)",
                    results.file_matches.len()
                ));
            }
        }
        OutputMode::Count => {
            let file_count = results.file_matches.len();
            output = format!(
                "{} matches across {} files",
                results.total_match_count, file_count
            );
        }
    }

    if results.total_match_count >= max_total_matches {
        output.push_str(&format!(
            "\n... (search stopped at {} matches)", max_total_matches
        ));
    }
    output
}
